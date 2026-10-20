//! O3 (OPTIMO.md §4): the in-app co-writer at work. It replaced phase 8's Assistant panel.
//!
//! * A run starts from ⌘Y (the Co-writer group, the "Ask the co-writer" row) or the context
//!   menu's Ask. The verb's prompt (`wordcraft_engine::cowriter::prompt`) goes to the provider
//!   chosen in Settings › Co-writer on a background thread; [`poll`] collects the stream each
//!   frame. A verb that needs no model runs at once.
//! * The answer is read and guarded by `wordcraft_engine::cowriter::run`, and lands through
//!   `cowriter::apply` (comments and tracked changes under the AI author, `run_as_assistant`).
//!   An ask lands as notes only (rule 1). Notes open the margin view, never Review.
//! * While a run works the bottom line says "Co-writer is reading… · Stop" ([`presence_line`]).
//!   Stop drops the stream and everything that would have come of it.
//! * Nothing is sent while "Let a co-writer work in Optimo" is off or no provider is usable.
//! * Each document keeps its runs for the session (in memory): the margin shows the last one's
//!   summary, the notes' kinds, and Review groups a multi-piece replacement by them.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use egui::{Color32, Rect, Stroke, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_assistant::{AssistantError, ChatReply};
use wordcraft_engine::assistant_edits::{self as edits, Applied, ScopeKind, ScopeText};
use wordcraft_engine::cowriter::{self, Call, Reply, Verb};
use wordcraft_engine::doc::Pos;

use crate::WordApp;
use crate::review_panel::mark_line;

/// How long connecting and the first words of the answer may take (local models read slowly).
const ASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// Runs kept per document.
const MAX_RUNS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Working,
    Done,
    Stopped,
    Failed,
}

/// One run and what came of it.
#[derive(Clone, Debug)]
pub struct Run {
    pub time: String,
    pub verb: Verb,
    /// What the person asked (Ask), else empty.
    pub instruction: String,
    /// "Selection · 12 words", "This section “…”", "Whole document".
    pub scope: String,
    /// The AI author it worked under.
    pub author: String,
    pub status: Status,
    /// The model's summary.
    pub report: String,
    /// What was refused, skipped or went wrong.
    pub lines: Vec<String>,
    /// The comments it made, with their kind ("Note", "Question"…).
    pub notes: Vec<(u32, String)>,
    /// Everything it landed, in document order.
    pub applied: Vec<Applied>,
    /// What the model sent when it couldn't be read (this session only, never saved).
    pub answer: Option<String>,
}

enum Event {
    Text(String),
    Done(Result<ChatReply, AssistantError>),
}

struct Job {
    doc: u64,
    run: usize,
    verb: Verb,
    scope: ScopeText,
    options: Value,
    rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
    buf: String,
    /// Complete quotes seen in the streamed answer, and where the last one sits.
    quotes: usize,
    presence: Option<Pos>,
    name: String,
    /// The provider asked (JSON mode is remembered per provider).
    provider: wordcraft_assistant::ProviderId,
}

/// The co-writer's state.
#[derive(Default)]
pub struct CoWriter {
    /// The margin view (notes in the right desk margin) is open: the top bar's Co-writer toggle.
    pub margin: bool,
    /// The last run's summary was hidden in the margin (a new run shows it again).
    pub hide_summary: bool,
    runs: HashMap<u64, Vec<Run>>,
    job: Option<Job>,
    /// Providers whose server refused JSON mode this session: not asked for it again.
    json_refused: HashSet<wordcraft_assistant::ProviderId>,
    /// O4: the view of the document's last Format for me run (`format_view`).
    pub format: Option<crate::format_view::FormatView>,
    /// O4: the saved looks, read once from `looks_file`.
    pub saved_looks: Option<Vec<wordcraft_engine::looks::Look>>,
    /// O4: `looks.json` in the settings folder (set by the desktop app; none on the web).
    pub looks_file: Option<std::path::PathBuf>,
    /// O5: the Alternatives card open under the selected sentence (`alternatives_view`).
    pub alts: Option<crate::alternatives_view::Card>,
    /// O9: the view of the empty paragraph I'm stuck in (`stuck_view`), and the paragraph a
    /// caller (Structure) asked it for.
    pub stuck: Option<crate::stuck_view::StuckView>,
    pub stuck_at: Option<wordcraft_engine::doc::Path>,
    /// O6: Read as a reader's chooser and Check's one-click options (`reader_view`).
    pub reader: crate::reader_view::State,
}

fn now_hm() -> String {
    // UTC, like the comment and change stamps in Review.
    wordcraft_engine::cmd::now_iso().get(11..16).unwrap_or("").to_string()
}

impl CoWriter {
    /// The document's runs, oldest first.
    pub fn runs(&self, doc: u64) -> &[Run] {
        self.runs.get(&doc).map(Vec::as_slice).unwrap_or_default()
    }

    pub fn working(&self) -> bool {
        self.job.is_some()
    }

    /// The kind line of note `id` ("Note" when this session didn't make it).
    pub fn kind(&self, doc: u64, id: u32) -> &str {
        self.runs(doc).iter().rev().flat_map(|r| r.notes.iter()).find(|(n, _)| *n == id).map_or("Note", |(_, k)| k.as_str())
    }
}

/// Why nothing may be sent right now, if so.
fn refusal(app: &mut WordApp) -> Option<String> {
    if !app.ui.assistant.enabled {
        return Some(tl!("Turn on “Let a co-writer work in Optimo” in Settings › Co-writer first: nothing is sent anywhere while it is off.").into());
    }
    if !app.assistant_link.usable(&app.ui.assistant) {
        if app.assistant_link.looking_up(app.ui.assistant.provider_id()) {
            return Some(tl!("Waiting for the keychain to hand over the co-writer's key: answer its prompt, then try again.").into());
        }
        return Some(tl!("Finish Settings › Co-writer first: choose a provider and add its key.").into());
    }
    None
}

/// Ask the co-writer `instruction` about `kind`: notes in the margin, never inserted text.
pub fn ask(app: &mut WordApp, instruction: &str, kind: ScopeKind) -> Result<Value, String> {
    start(app, Verb::Ask, kind, json!({"instruction": instruction.trim()}))
}

/// Start `verb` on `kind` with its `options`. Verbs that need a model run in the background
/// ([`poll`]); the others land at once.
pub fn start(app: &mut WordApp, verb: Verb, kind: ScopeKind, mut options: Value) -> Result<Value, String> {
    // O6: Read asks who the reader is first; Check gets the allowed folders.
    if let Some(r) = crate::reader_view::before_start(app, verb, kind, &mut options) {
        return r;
    }
    let p = app.ui.assistant.provider_id();
    let model = app.ui.assistant.model(p).map(str::to_string);
    let mut kind = kind;
    if verb == Verb::Alternatives {
        // O5: the selection, else the sentence at the caret.
        kind = ScopeKind::Selection;
        if app.session.sel.is_collapsed() {
            let _ = app.session.run("select.sentence", &json!({}));
        }
    }
    if verb == Verb::FormatForMe {
        // O4: the selection, else the whole document; rule-based, so it works without a model
        // (and sends nothing): then the look is Clean.
        if kind == ScopeKind::Section {
            kind = ScopeKind::Document;
        }
        let given = options.get("look").is_some_and(|l| !l.is_null());
        if !given && (refusal(app).is_some() || model.is_none()) {
            match options.as_object_mut() {
                Some(m) => {
                    m.insert("look".into(), json!("clean"));
                }
                None => options = json!({"look": "clean"}),
            }
        }
    }
    let needs_model = !(verb == Verb::FormatForMe && options.get("look").is_some_and(|l| !l.is_null()));
    if needs_model && let Some(why) = refusal(app) {
        return Err(why);
    }
    if app.cowriter.job.is_some() {
        return Err(tl!("Still working on the last request: wait, or Stop it first.").into());
    }
    let model = match model {
        Some(m) => m,
        None if !needs_model => String::new(),
        None => return Err(tl!("Choose a model in Settings › Co-writer first.").into()),
    };
    let scope = if verb == Verb::Stuck {
        // O9: the caret goes into an empty paragraph first; an empty document is still a scope.
        crate::stuck_view::begin(app)?
    } else {
        edits::scope(&app.session, kind).map_err(|e| capitalise(&e))?
    };
    let name = app.ui.assistant.name();
    let prompt = cowriter::prompt(verb, &Call { session: &app.session, scope: &scope, options: &options, name: &name })?;
    if verb == Verb::Alternatives {
        crate::alternatives_view::begin(app, &scope, &options);
    }
    let doc = app.session.document_id();
    let mut run = Run {
        time: now_hm(),
        verb,
        instruction: options.get("instruction").and_then(Value::as_str).unwrap_or_default().chars().take(edits::MAX_INSTRUCTION_CHARS).collect(),
        scope: scope_label(&scope),
        author: name.clone(),
        status: Status::Working,
        report: String::new(),
        lines: Vec::new(),
        notes: Vec::new(),
        applied: Vec::new(),
        answer: None,
    };
    if scope.trimmed {
        run.lines.push(crate::i18n::fmt(
            tl!("Only the first {n} words were sent: the scope is longer than one run can take. Notes can only land there."),
            &[("n", &scope.words().to_string())],
        ));
    }
    let Some(prompt) = prompt else {
        // No model needed: read, guard and land now.
        push_run(app, doc, run);
        let index = app.cowriter.runs(doc).len().saturating_sub(1);
        let job_less = cowriter::run(verb, &Call { session: &app.session, scope: &scope, options: &options, name: &name }, None);
        land(app, doc, index, verb, &scope, &name, job_less, None, &options);
        return Ok(json!({"started": true, "done": true, "scope": kind.id()}));
    };
    let request = wordcraft_assistant::ChatRequest {
        model,
        system: Some(prompt.system),
        messages: vec![wordcraft_assistant::Message { role: wordcraft_assistant::Role::User, content: prompt.user }],
        max_tokens: Some(prompt.max_tokens),
        // JSON mode where the server takes it (Mistral, OpenAI, OpenRouter, Ollama…).
        json: !app.cowriter.json_refused.contains(&p),
    };
    let (rx, stop) = spawn(app, request)?;
    push_run(app, doc, run);
    let index = app.cowriter.runs(doc).len().saturating_sub(1);
    let presence = scope.start();
    app.cowriter.job = Some(Job { doc, run: index, verb, scope, options, rx, stop, buf: String::new(), quotes: 0, presence, name, provider: p });
    Ok(json!({"started": true, "scope": kind.id()}))
}

fn push_run(app: &mut WordApp, doc: u64, run: Run) {
    let list = app.cowriter.runs.entry(doc).or_default();
    list.push(run);
    if list.len() > MAX_RUNS {
        list.remove(0);
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// The scope as a run names it: "Selection · 12 words", "This section “Why share a studio?”",
/// "Whole document".
fn scope_label(s: &ScopeText) -> String {
    match (&s.title, s.kind) {
        (Some(t), ScopeKind::Section) => format!("{} “{}”", tl!(s.kind.label()), crate::review_panel::clip(t, 40)),
        (_, ScopeKind::Selection) => {
            format!("{} · {}", tl!(s.kind.label()), crate::i18n::fmt(tl!("{n} words"), &[("n", &s.words().to_string())]))
        }
        _ => tl!(s.kind.label()).to_string(),
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn(_app: &mut WordApp, _req: wordcraft_assistant::ChatRequest) -> Result<(Receiver<Event>, Arc<AtomicBool>), String> {
    Err(tl!("The co-writer works in the Optimo desktop app.").into())
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(app: &mut WordApp, req: wordcraft_assistant::ChatRequest) -> Result<(Receiver<Event>, Arc<AtomicBool>), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let keys = app.assistant_link.key_store();
    let p = app.ui.assistant.provider_id();
    let url = app.ui.assistant.base_url(p);
    let wake = app.ctx.clone();
    let flag = stop.clone();
    let spawned = std::thread::Builder::new().name("cowriter-run".into()).spawn(move || {
        // The key is read here, off the UI thread.
        let answer = keys.get(p.id()).and_then(|key| wordcraft_assistant::Endpoint::new(p, Some(&url), key)).and_then(|ep| {
            let key = ep.api_key.clone();
            wordcraft_assistant::http::chat_stream_until(&ep, &req, ASK_TIMEOUT, &flag, &mut |t| {
                // A send error means the run was let go (Stop): the flag ends the stream.
                if tx.send(Event::Text(t.to_string())).is_err() {
                    flag.store(true, Ordering::Relaxed);
                }
                if let Some(c) = &wake {
                    c.request_repaint();
                }
            })
            .map(|mut reply| {
                // The answer may be shown: never with the key in it.
                if let Some(k) = key.as_deref().filter(|k| k.len() >= 4) {
                    reply.text = reply.text.replace(k, "••••");
                }
                reply
            })
        });
        let _ = tx.send(Event::Done(answer));
        if let Some(c) = &wake {
            c.request_repaint();
        }
    });
    match spawned {
        Ok(_) => Ok((rx, stop)),
        Err(e) => Err(format!("{} {e}", tl!("Could not start the request:"))),
    }
}

/// Record the kind line ("Question") of a note made outside a run's landing.
pub(crate) fn remember_note(app: &mut WordApp, id: u32, kind: &str) {
    let doc = app.session.document_id();
    if let Some(r) = app.cowriter.runs.get_mut(&doc).and_then(|l| l.iter_mut().rev().find(|r| r.verb == Verb::Stuck)) {
        r.notes.push((id, kind.to_string()));
        return;
    }
    // No run (the control hook): a quiet one holds the kind.
    let author = app.ui.assistant.name();
    push_run(
        app,
        doc,
        Run {
            time: now_hm(),
            verb: Verb::Stuck,
            instruction: String::new(),
            scope: String::new(),
            author,
            status: Status::Done,
            report: String::new(),
            lines: Vec::new(),
            notes: vec![(id, kind.to_string())],
            applied: Vec::new(),
            answer: None,
        },
    );
}

pub(crate) fn run_mut(app: &mut WordApp, doc: u64, i: usize) -> Option<&mut Run> {
    app.cowriter.runs.get_mut(&doc).and_then(|l| l.get_mut(i))
}

/// Stop: drop the stream and discard what it would have brought.
pub fn stop(app: &mut WordApp) -> bool {
    let Some(job) = app.cowriter.job.take() else { return false };
    job.stop.store(true, Ordering::Relaxed);
    app.cowriter.alts = None;
    if let Some(r) = run_mut(app, job.doc, job.run) {
        r.status = Status::Stopped;
        r.report.clear();
        r.lines.push(tl!("Stopped. Nothing was added.").into());
    }
    true
}

/// Each frame: collect what the answer brought, and land it when complete.
pub fn poll(app: &mut WordApp) {
    crate::stuck_view::tick(app);
    let Some(job) = app.cowriter.job.as_mut() else { return };
    if job.doc != app.session.document_id() {
        let (doc, i) = (job.doc, job.run);
        stop(app);
        if let Some(r) = run_mut(app, doc, i) {
            r.lines.push(tl!("The document was closed or replaced while it worked.").into());
        }
        return;
    }
    let mut done = None;
    let mut grew = false;
    loop {
        match job.rx.try_recv() {
            Ok(Event::Text(t)) => {
                if job.buf.len() < edits::MAX_ANSWER_CHARS {
                    job.buf.push_str(&t);
                }
                grew = true;
            }
            Ok(Event::Done(r)) => {
                done = Some(r);
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done = Some(Err(AssistantError::Protocol("the connection stopped".into())));
                break;
            }
        }
    }
    if grew {
        // The presence follows the last complete quote that sits in the scope.
        let quotes: Vec<String> = edits::partial_strings(&job.buf, "quote").into_iter().filter(|(_, c)| *c).map(|(q, _)| q).collect();
        if quotes.len() > job.quotes {
            job.quotes = quotes.len();
            if let Some(f) = quotes.last().and_then(|q| edits::locate(&app.session, &job.scope, q).ok()) {
                job.presence = Some(Pos { story: job.scope.story, path: f.path, off: f.end });
            }
        }
        let (doc, i) = (job.doc, job.run);
        if let Some(report) = edits::partial_report(&job.buf)
            && let Some(r) = run_mut(app, doc, i)
        {
            r.report = report;
        }
    }
    if let Some(r) = done {
        finish(app, r);
    }
}

fn finish(app: &mut WordApp, r: Result<ChatReply, AssistantError>) {
    let Some(job) = app.cowriter.job.take() else { return };
    if job.verb == Verb::Alternatives {
        // O5: the answer fills the card; nothing lands until a row is used.
        crate::alternatives_view::arrived(app, job.doc, job.run, &job.scope, &job.options, &job.name, r);
        return;
    }
    match r {
        Ok(reply) => {
            if reply.json_refused {
                app.cowriter.json_refused.insert(job.provider);
            }
            let call = Call { session: &app.session, scope: &job.scope, options: &job.options, name: &job.name };
            let ran = cowriter::run(job.verb, &call, Some(Reply { text: &reply.text, stop_reason: reply.stop_reason.as_deref() }));
            land(app, job.doc, job.run, job.verb, &job.scope, &job.name, ran, Some(reply.text), &job.options);
        }
        Err(e) if job.verb == Verb::FormatForMe && !matches!(e, AssistantError::Stopped) => {
            // O4: the look guess failed; the formatting itself needs no model: Clean.
            log::warn!("co-writer: look guess: {e}");
            let mut options = job.options.clone();
            if let Some(m) = options.as_object_mut() {
                m.insert("look".into(), json!("clean"));
            }
            let call = Call { session: &app.session, scope: &job.scope, options: &options, name: &job.name };
            let ran = cowriter::run(job.verb, &call, None).map(|mut r| {
                r.lines.insert(0, format!("{} {}", tl!("Couldn’t ask the co-writer for a look, so it is Clean:"), e));
                r
            });
            land(app, job.doc, job.run, job.verb, &job.scope, &job.name, ran, None, &options);
        }
        Err(e) => {
            let stopped = matches!(e, AssistantError::Stopped);
            if !stopped {
                // Never holds the key (wordcraft_assistant::AssistantError).
                log::warn!("co-writer: {e}");
            }
            if let Some(run) = run_mut(app, job.doc, job.run) {
                run.status = if stopped { Status::Stopped } else { Status::Failed };
                if !stopped {
                    run.lines.push(capitalise(&e.to_string()));
                }
            }
        }
    }
}

/// Land a read answer in the document and record it in run `i`. Notes open the margin view.
#[allow(clippy::too_many_arguments)]
fn land(
    app: &mut WordApp,
    doc: u64,
    i: usize,
    verb: Verb,
    scope: &ScopeText,
    name: &str,
    ran: Result<cowriter::Ran, String>,
    raw: Option<String>,
    options: &Value,
) {
    crate::assistant_link::sync_policy(app);
    // O9: I'm stuck keeps its questions and first line in the margin view, landing none of them.
    let ran = if verb == Verb::Stuck { ran.map(|r| crate::stuck_view::take(app, r)) } else { ran };
    // O4: Format for me keeps the document as it was, for Before and ⌥ compare.
    let before = (verb == Verb::FormatForMe).then(|| app.session.doc.clone());
    let (status, report, mut lines, notes, applied, answer) = match ran {
        Ok(ran) => {
            let outputs = if before.is_some() { ran.outputs.clone() } else { Vec::new() };
            crate::reader_view::remember(app, doc, &ran.outputs);
            let landed = cowriter::apply(&mut app.session, name, verb, scope, ran.outputs);
            if let Some(b) = before {
                crate::format_view::landed(app, &outputs, &ran.report, ran.lines.clone(), landed.words, b, options, scope.kind);
            }
            let mut lines = ran.lines;
            lines.extend(landed.lines);
            if landed.applied.is_empty() && ran.report.trim().is_empty() {
                lines.push(tl!("No notes this time.").into());
            }
            if !landed.applied.is_empty() {
                app.ui.assistant.remember_name(name);
            }
            // A line about a cut-off answer keeps the raw answer for a look.
            let keep = lines.iter().any(|l| l.contains("cut off") || l.contains("stopped early")).then_some(raw).flatten();
            (Status::Done, ran.report, lines, landed.notes, landed.applied, keep)
        }
        Err(e) => {
            log::warn!("co-writer: answer not usable ({} characters)", raw.as_deref().map_or(0, str::len));
            let keep = raw.filter(|t| !t.trim().is_empty());
            (Status::Failed, String::new(), vec![e], Vec::new(), Vec::new(), keep)
        }
    };
    let opened = !notes.is_empty() || !report.trim().is_empty();
    if let Some(r) = run_mut(app, doc, i) {
        r.status = status;
        if !report.is_empty() || status != Status::Done {
            r.report = report;
        }
        r.lines.append(&mut lines);
        r.notes = notes;
        r.applied = applied;
        r.answer = answer.map(|t| t.chars().take(edits::MAX_ANSWER_CHARS).collect());
    }
    app.cowriter.hide_summary = false;
    if opened {
        crate::mac_ui::set_margin(app, true);
    }
}

/// The pending replacements of the current document made of several word-level pieces:
/// (author, parts, what the row shows deleted, inserted). Review shows each as one row.
pub(crate) fn pending_groups(app: &WordApp) -> Vec<(String, Vec<(String, String)>, String, String)> {
    let doc = app.session.document_id();
    app.cowriter
        .runs(doc)
        .iter()
        .flat_map(|r| {
            r.applied.iter().filter(|a| a.parts.len() > 1).map(|a| (r.author.clone(), a.parts.clone(), a.deleted.clone(), a.inserted.clone()))
        })
        .collect()
}

/// Where `parts` sit in `list` (one author's pending changes, in document order, as
/// `review_panel::changes_by` gives them): the first and last index of a run of consecutive
/// changes in one paragraph matching them in order.
pub(crate) fn find_group(list: &[(String, String, Pos, Pos)], parts: &[(String, String)]) -> Option<(usize, usize)> {
    let n = parts.len();
    if n == 0 || n > list.len() {
        return None;
    }
    (0..=list.len() - n).find_map(|i| {
        let first = list.get(i)?;
        let ok = parts.iter().enumerate().all(|(k, (d, ins))| {
            list.get(i + k).is_some_and(|(cd, ci, a, _)| cd == d && ci == ins && a.story == first.2.story && a.path == first.2.path)
        });
        ok.then_some((i, i + n - 1))
    })
}

/// Where the co-writer is reading on the page while it works: the caret, and the block around it.
pub fn presence(app: &WordApp) -> Option<(Pos, Pos, Pos)> {
    let job = app.cowriter.job.as_ref()?;
    if job.doc != app.session.document_id() {
        return None;
    }
    let caret = app.session.doc.clamp(job.presence.as_ref()?);
    let len = app.session.doc.para_at(&caret).map_or(0, |p| p.len());
    let a = Pos { off: 0, ..caret.clone() };
    let b = Pos { off: len, ..caret.clone() };
    Some((caret, a, b))
}

/// §4 presence for the bottom line while a run works: "Co-writer is reading…".
pub fn presence_line(app: &WordApp) -> Option<String> {
    app.cowriter.job.as_ref().map(|_| tl!("Co-writer is reading…").to_string())
}

fn run_json(app: &WordApp, r: &Run) -> Value {
    let status = match r.status {
        Status::Working => "working",
        Status::Done => "done",
        Status::Stopped => "stopped",
        Status::Failed => "failed",
    };
    let notes: Vec<Value> = r
        .notes
        .iter()
        .map(|(id, kind)| {
            let a = r.applied.iter().find(|a| a.comment == Some(*id));
            let open = app.session.doc.comments.get(id).is_some_and(|c| !c.resolved);
            json!({"id": id, "kind": kind, "quote": a.map(|a| a.quote.clone()), "text": a.map(|a| a.note.clone()), "open": open})
        })
        .collect();
    json!({"time": r.time, "verb": r.verb.id(), "instruction": r.instruction, "scope": r.scope, "status": status, "report": r.report, "lines": r.lines, "notes": notes, "changes": r.applied.iter().filter(|a| a.comment.is_none()).count(), "answer": r.answer})
}

/// `ui.assistant.ask` / `.run` / `.stop` / `.panel` / `.log`; `None` for other ids. All are the
/// person's: outside assistants are refused every `ui.assistant.*`
/// (`wordcraft_engine::assistant::denied`).
///
/// * `ui.assistant.ask` `{"instruction": string, "scope"?: "selection" | "section" | "document"}`
///   (default: the selection when there is one, else the section at the caret): notes only;
/// * `ui.assistant.run` `{"verb": "format" | …, "scope"?, …options}`: an available verb;
/// * `ui.assistant.stop`;
/// * `ui.assistant.formatView` `{"compare"?: bool, "show"?: what, "action"?: "keep" | "undoAll" |
///   "oneByOne", "option"?: "keepColours" | "toc" | "numberHeadings", "value"?: bool}`: Format
///   for me's view (O4);
/// * `ui.assistant.alternatives` `{"pick"?: 1–3, "text"?: string, "close"?: bool}`: the open
///   Alternatives card (O5): use row `pick` (or `text`, as if edited first), or close it; no
///   argument reads its rows;
/// * `ui.assistant.panel` `{"value"?: bool}`: the margin view (toggles without a value);
/// * `ui.assistant.log`: this document's runs and whether one is working.
pub fn command(app: &mut WordApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    let kind = |app: &WordApp| -> Result<ScopeKind, String> {
        match p.get("scope") {
            None | Some(Value::Null) => Ok(if app.session.sel.is_collapsed() { ScopeKind::Section } else { ScopeKind::Selection }),
            Some(v) => v.as_str().and_then(ScopeKind::from_id).ok_or_else(|| "`scope` must be selection, section or document".to_string()),
        }
    };
    Some(match id {
        "ui.assistant.ask" => {
            let instruction = p.get("instruction").and_then(Value::as_str).unwrap_or_default().to_string();
            kind(app).and_then(|k| ask(app, &instruction, k))
        }
        "ui.assistant.run" => {
            let verb = p.get("verb").and_then(Value::as_str).and_then(Verb::from_id);
            match verb {
                Some(v) => kind(app).and_then(|k| start(app, v, k, p.clone())),
                None => Err("`verb` must be one of the co-writer's verbs".into()),
            }
        }
        "ui.assistant.stop" => Ok(json!({"stopped": stop(app)})),
        "ui.assistant.formatView" => crate::format_view::control(app, p),
        "ui.assistant.alternatives" => crate::alternatives_view::control(app, p),
        "ui.assistant.stuckView" => crate::stuck_view::control(app, p),
        "ui.assistant.panel" => {
            let on = match p.get("value") {
                None | Some(Value::Null) => !app.cowriter.margin,
                Some(v) => match v.as_bool() {
                    Some(b) => b,
                    None => return Some(Err("`value` must be true or false".into())),
                },
            };
            app.cowriter.margin = on;
            Ok(json!({"open": on}))
        }
        "ui.assistant.log" => {
            poll(app);
            let doc = app.session.document_id();
            let list: Vec<Value> = app.cowriter.runs(doc).iter().map(|r| run_json(app, r)).collect();
            Ok(json!({"working": app.cowriter.working(), "entries": list}))
        }
        _ => return None,
    })
}

/// On the page while a run works: a 2 pt caret in `assistant` with a name tag (11 pt, the one
/// size exception), `assistant_tint` over the block it reads, and a dotted bar beside it.
pub fn paint_presence(app: &WordApp, painter: &egui::Painter, rects: &[Rect], layout: &wordcraft_layout::DocLayout, scale: f32) {
    let Some((caret, a, b)) = presence(app) else { return };
    let color = crate::skin::ASSISTANT_PAGE;
    let page_scale = |pi: usize, pr: Rect| layout.pages.get(pi).map_or(scale, |page| crate::canvas::page_screen_scale(pr, page, scale));
    let mut block: Option<Rect> = None;
    for (pi, r) in layout.selection_rects(&app.session.doc, &a, &b, app.session.page_hint) {
        let Some(pr) = rects.get(pi) else { continue };
        let s = page_scale(pi, *pr);
        let sr = Rect::from_min_size(pos2(pr.min.x + r.x * s, pr.min.y + r.y * s), vec2(r.w * s, r.h * s));
        painter.rect_filled(sr, 0.0, crate::skin::ASSISTANT_TINT);
        block = Some(block.map_or(sr, |x| x.union(sr)));
    }
    if let Some(bk) = block {
        let x = bk.min.x - 14.0;
        mark_line(painter, pos2(x, bk.min.y), pos2(x, bk.max.y), color, true);
    }
    let Some(c) = layout.caret_on(&caret, app.session.page_hint) else { return };
    let Some(pr) = rects.get(c.page) else { return };
    let s = page_scale(c.page, *pr);
    let x = pr.min.x + c.x * s;
    let y0 = pr.min.y + c.top * s;
    let y1 = y0 + c.height * s;
    painter.line_segment([pos2(x, y0), pos2(x, y1)], Stroke::new(2.0, color));
    let name = crate::review_panel::shown_assistant_name();
    let g = painter.layout_no_wrap(name, crate::theme::regular(11.0), Color32::WHITE);
    let tag = Rect::from_min_size(pos2(x - 1.0, y0 - g.size().y - 4.0), g.size() + vec2(8.0, 4.0));
    painter.rect_filled(tag, 3.0, color);
    painter.galley(tag.min + vec2(4.0, 2.0), g, Color32::WHITE);
}

#[cfg(test)]
mod tests;
