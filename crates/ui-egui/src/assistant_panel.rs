//! Phase 8 (HANDOFF §8, A1–A5): the Assistant panel, and asking the built-in assistant.
//!
//! * The person asks (composer, command field ⌘Y, the selection's "Ask …" item). The scope's
//!   text and the request go to the provider chosen in Settings › Assistant on a background
//!   thread; the answer streams into the work log ([`poll`]).
//! * The answer is read and applied by `wordcraft_engine::assistant_edits`: every edit becomes
//!   its own tracked change (or comment) under the assistant's author name, through
//!   `Session::run_as_assistant`, the same choke point and deny list outside assistants go
//!   through. Nothing is accepted until the person accepts it here or in Review.
//! * "Check facts against a file…" (K4) sends a second untrusted block, the text of a file inside
//!   the folders allowed in Settings › Assistant (read-only, see
//!   `wordcraft_engine::assistant_source`), and only comments land.
//! * Nothing is sent while the S1 switch is off or no provider is usable. Stop drops the stream
//!   and everything that would have come of it.
//! * The work log is kept per document for the session (in memory, not saved).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use egui::{Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_assistant::{AssistantError, ChatReply};
use wordcraft_engine::assistant_edits::{self as edits, Applied, EditKind, ScopeKind, ScopeText};
use wordcraft_engine::assistant_source as source;
use wordcraft_engine::doc::Pos;

use crate::WordApp;
use crate::mac_controls as mc;
use crate::review_panel::{Deco, mark_line};
use crate::skin::MarginTokens;
use crate::theme::Tokens;

/// The composer's instruction box (A4).
pub const COMPOSER_H: f32 = 76.0;
/// Suggestion rows (A3).
pub const SUGGESTION_H: f32 = 36.0;
/// How long connecting and the first words of the answer may take (local models read slowly).
const ASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// Entries kept per document.
const MAX_ENTRIES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Working,
    Done,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SugState {
    Pending,
    Accepted,
    Rejected,
    /// Accepted, rejected or edited away elsewhere (Review, undo, typing).
    Gone,
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub applied: Applied,
    pub state: SugState,
}

/// One ask and its reply (A2).
#[derive(Clone, Debug)]
pub struct Entry {
    pub time: String,
    pub instruction: String,
    /// "Selection", "This section", "Whole document".
    pub scope: String,
    /// The author name the edits were made under.
    pub author: String,
    pub reply_time: String,
    pub status: Status,
    pub report: String,
    /// Plain lines under the report: what was trimmed, skipped or went wrong.
    pub lines: Vec<String>,
    pub suggestions: Vec<Suggestion>,
    /// What the model sent, kept (in memory, this session only, never saved) when it couldn't be
    /// read in full, for "Show answer". Keys are taken out before it gets here.
    pub answer: Option<String>,
    /// "Show answer" is open.
    pub show_answer: bool,
}

enum Event {
    Text(String),
    Done(Result<ChatReply, AssistantError>),
}

struct Job {
    doc: u64,
    entry: usize,
    scope: ScopeText,
    rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
    buf: String,
    /// Complete quotes seen in the streamed answer, and where the last one sits.
    quotes: usize,
    presence: Option<Pos>,
    name: String,
    /// A fact check: only comments are made.
    facts: bool,
    /// The provider asked (JSON mode is remembered per provider).
    provider: wordcraft_assistant::ProviderId,
}

/// The list of files "Check facts against a file…" offers, drawn as a window until one is
/// picked or the window is closed.
pub struct FilePicker {
    /// What to ask about the file.
    pub instruction: String,
    pub files: Vec<std::path::PathBuf>,
    /// The scope the ask was opened with.
    pub scope: ScopeKind,
}

/// The panel's state.
#[derive(Default)]
pub struct AssistantPanel {
    pub open: bool,
    /// The composer's Scope choice.
    pub scope: ScopeKind,
    pub draft: String,
    logs: HashMap<u64, Vec<Entry>>,
    job: Option<Job>,
    /// The file list of "Check facts against a file…" while it is open.
    pub picker: Option<FilePicker>,
    /// (document, revision) the suggestion states were last checked at.
    checked: (u64, u64),
    /// Providers whose server refused JSON mode this session: not asked for it again.
    json_refused: std::collections::HashSet<wordcraft_assistant::ProviderId>,
}

fn now_hm() -> String {
    // UTC, like the comment and change stamps in Review.
    wordcraft_engine::cmd::now_iso().get(11..16).unwrap_or("").to_string()
}

impl AssistantPanel {
    /// The current document's work log.
    pub fn log(&self, doc: u64) -> &[Entry] {
        self.logs.get(&doc).map(Vec::as_slice).unwrap_or_default()
    }

    pub fn working(&self) -> bool {
        self.job.is_some()
    }
}

/// Whether the panel shows this frame: opened, an assistant usable, the Mac layout, and no other
/// right panel took its place.
pub fn visible(app: &WordApp) -> bool {
    app.assistant_panel.open && crate::review_panel::assistant_enabled() && crate::mac_ui::active(app) && !app.session.view.comments_pane
}

/// Open or close the panel. Opening closes the Format inspector and Review (one right panel at
/// a time) and picks the selection as scope when there is one.
pub fn set_open(app: &mut WordApp, on: bool) {
    app.assistant_panel.open = on;
    if on {
        app.session.view.comments_pane = false;
        app.ui.margin.inspector = false;
        if !app.session.sel.is_collapsed() {
            app.assistant_panel.scope = ScopeKind::Selection;
        } else if app.assistant_panel.scope == ScopeKind::Selection {
            // Nothing selected: the part where the caret is (Whole document only when picked).
            app.assistant_panel.scope = ScopeKind::Section;
        }
    }
}

/// Why nothing may be sent right now, if so.
fn refusal(app: &mut WordApp) -> Option<String> {
    if !app.ui.assistant.enabled {
        return Some(
            tl!("Turn on “Let an assistant work in Margin” in Settings › Assistant first: nothing is sent anywhere while it is off.").into(),
        );
    }
    if !app.assistant_link.usable(&app.ui.assistant) {
        return Some(tl!("Finish Settings › Assistant first: choose a provider and add its key.").into());
    }
    None
}

/// Ask the assistant: `instruction` about `kind`. Opens the panel; the answer arrives in [`poll`].
pub fn ask(app: &mut WordApp, instruction: &str, kind: ScopeKind) -> Result<Value, String> {
    ask_with(app, instruction, kind, None)
}

/// "Check facts against a file…": [`ask`], with the text of the file at `path` (which must lie
/// inside the allowed folders) as a second untrusted block; only comments come back.
pub fn ask_file(app: &mut WordApp, instruction: &str, kind: ScopeKind, path: &str) -> Result<Value, String> {
    ask_with(app, instruction, kind, Some(path))
}

/// The ask the file picker sends when the person didn't write one.
pub const FACT_CHECK: &str = "Check the facts in this against the file.";

fn ask_with(app: &mut WordApp, instruction: &str, kind: ScopeKind, file: Option<&str>) -> Result<Value, String> {
    if let Some(why) = refusal(app) {
        return Err(why);
    }
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err(tl!("Write what you want first.").into());
    }
    if app.assistant_panel.job.is_some() {
        return Err(tl!("Still working on the last request: wait, or Stop it first.").into());
    }
    let p = app.ui.assistant.provider_id();
    let Some(model) = app.ui.assistant.model(p).map(str::to_string) else {
        return Err(tl!("Choose a model in Settings › Assistant first.").into());
    };
    let scope = edits::scope(&app.session, kind).map_err(|e| capitalise(&e))?;
    let source = match file {
        None => None,
        Some(path) => Some(source::read(path, &app.ui.assistant.allowed_folders()).map_err(|e| capitalise(&e))?),
    };
    crate::assistant_link::sync_policy(app);
    let name = app.ui.assistant.name();
    let doc = app.session.document_id();
    let mut entry = Entry {
        time: now_hm(),
        instruction: instruction.chars().take(edits::MAX_INSTRUCTION_CHARS).collect(),
        scope: match &source {
            Some(src) => format!("{} · {}", scope_label(&scope), src.name),
            None => scope_label(&scope),
        },
        author: name.clone(),
        reply_time: String::new(),
        status: Status::Working,
        report: String::new(),
        lines: Vec::new(),
        suggestions: Vec::new(),
        answer: None,
        show_answer: false,
    };
    if scope.trimmed {
        entry.lines.push(crate::i18n::fmt(
            tl!("Only the first {n} words were sent: the scope is longer than an ask can take. Edits can only land there."),
            &[("n", &scope.words().to_string())],
        ));
    }
    if let Some(src) = source.as_ref().filter(|s| s.trimmed) {
        // Same kind of line as the scope's: say what the model did not see.
        log::info!("assistant: source {} trimmed to {} of {} characters", src.name, src.text.chars().count(), src.total_chars);
        entry.lines.push(crate::i18n::fmt(
            tl!("Only the first {n} words of {file} were sent: the file is longer than an ask can take."),
            &[("n", &src.words().to_string()), ("file", &src.name)],
        ));
    }
    let request = wordcraft_assistant::ChatRequest {
        model,
        system: Some(edits::system_prompt_for(&name, source.is_some())),
        messages: vec![wordcraft_assistant::Message {
            role: wordcraft_assistant::Role::User,
            content: edits::user_message_with(&entry.instruction, &scope, &edits::nonce(), source.as_ref()),
        }],
        // Room for the whole scope to be rewritten (quotes and replacements), within limits.
        max_tokens: Some(edits::output_budget(scope.text.chars().count())),
        // JSON mode where the server takes it (Mistral, OpenAI, OpenRouter, Ollama…).
        json: !app.assistant_panel.json_refused.contains(&p),
    };
    let (rx, stop) = spawn(app, request)?;
    let log = app.assistant_panel.logs.entry(doc).or_default();
    log.push(entry);
    if log.len() > MAX_ENTRIES {
        log.remove(0);
    }
    let index = log.len() - 1;
    let presence = scope.start();
    let facts = source.is_some();
    app.assistant_panel.job = Some(Job { doc, entry: index, scope, rx, stop, buf: String::new(), quotes: 0, presence, name, facts, provider: p });
    set_open(app, true);
    app.assistant_panel.scope = kind;
    Ok(json!({"started": true, "scope": kind.id()}))
}

/// Whether "Check facts against a file…" is offered: the assistant is usable and at least one
/// folder is allowed.
pub fn facts_available(app: &WordApp) -> bool {
    !app.ui.assistant.allowed_folders().is_empty()
}

/// Open the list of files in the allowed folders. The number of files listed; an error says why
/// there is nothing to pick from.
pub fn open_file_picker(app: &mut WordApp, kind: ScopeKind) -> Result<usize, String> {
    if let Some(why) = refusal(app) {
        return Err(why);
    }
    let folders = app.ui.assistant.allowed_folders();
    if folders.is_empty() {
        return Err(tl!("Allow a folder in Settings › Assistant first: “Read files in these folders”.").into());
    }
    let files = source::list(&folders);
    if files.is_empty() {
        return Err(tl!("No text files were found in the allowed folders.").into());
    }
    let n = files.len();
    app.assistant_panel.picker = Some(FilePicker { instruction: FACT_CHECK.into(), files, scope: kind });
    Ok(n)
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// The scope as the work log names it: "Selection · 12 words", "This section “Why share a
/// studio?”", "Whole document".
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
    Err(tl!("The assistant works in the Margin desktop app.").into())
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
    let spawned = std::thread::Builder::new().name("assistant-ask".into()).spawn(move || {
        // The key is read here, off the UI thread.
        let answer = keys.get(p.id()).and_then(|key| wordcraft_assistant::Endpoint::new(p, Some(&url), key)).and_then(|ep| {
            let key = ep.api_key.clone();
            wordcraft_assistant::http::chat_stream_until(&ep, &req, ASK_TIMEOUT, &flag, &mut |t| {
                // A send error means the panel let go (Stop): the flag ends the stream.
                if tx.send(Event::Text(t.to_string())).is_err() {
                    flag.store(true, Ordering::Relaxed);
                }
                if let Some(c) = &wake {
                    c.request_repaint();
                }
            })
            .map(|mut reply| {
                // The answer may be shown ("Show answer"): never with the key in it.
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

fn entry_mut(app: &mut WordApp, doc: u64, i: usize) -> Option<&mut Entry> {
    app.assistant_panel.logs.get_mut(&doc).and_then(|l| l.get_mut(i))
}

/// Stop: drop the stream, discard what it would have suggested, remove the presence.
pub fn stop(app: &mut WordApp) -> bool {
    let Some(job) = app.assistant_panel.job.take() else { return false };
    job.stop.store(true, Ordering::Relaxed);
    if let Some(e) = entry_mut(app, job.doc, job.entry) {
        e.status = Status::Stopped;
        e.reply_time = now_hm();
        e.lines.push(tl!("Stopped. Nothing was suggested.").into());
    }
    true
}

/// Each frame: collect what the answer brought, apply it when complete, and keep suggestion
/// states in step with the document.
pub fn poll(app: &mut WordApp) {
    let Some(job) = app.assistant_panel.job.as_mut() else {
        refresh_states(app);
        return;
    };
    if job.doc != app.session.document_id() {
        let doc = job.doc;
        stop(app);
        if let Some(e) = app.assistant_panel.logs.get_mut(&doc).and_then(|l| l.last_mut()) {
            e.lines.push(tl!("The document was closed or replaced while it worked.").into());
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
        let (doc, i) = (job.doc, job.entry);
        let report = edits::partial_report(&job.buf);
        // The presence follows the last complete quote that sits in the scope.
        let quotes: Vec<String> = edits::partial_strings(&job.buf, "quote").into_iter().filter(|(_, c)| *c).map(|(q, _)| q).collect();
        if quotes.len() > job.quotes {
            job.quotes = quotes.len();
            if let Some(f) = quotes.last().and_then(|q| edits::locate(&app.session, &job.scope, q).ok()) {
                job.presence = Some(Pos { story: job.scope.story, path: f.path, off: f.end });
            }
        }
        if let (Some(r), Some(e)) = (report, entry_mut(app, doc, i)) {
            e.report = r;
        }
    }
    if let Some(r) = done {
        finish(app, r);
    }
}

fn finish(app: &mut WordApp, r: Result<ChatReply, AssistantError>) {
    let Some(job) = app.assistant_panel.job.take() else { return };
    let mut lines = Vec::new();
    let mut report = String::new();
    let mut suggestions = Vec::new();
    let mut answer_text = None;
    let status = match r {
        Ok(reply) => {
            if reply.json_refused {
                app.assistant_panel.json_refused.insert(job.provider);
            }
            match edits::read_answer(&reply.text, reply.stop_reason.as_deref()) {
                Ok(answer) => {
                    report = answer.report;
                    lines.extend(answer.skipped);
                    crate::assistant_link::sync_policy(app);
                    let out = if job.facts {
                        edits::apply_comments_only(&mut app.session, &job.name, &job.scope, &answer.edits)
                    } else {
                        edits::apply(&mut app.session, &job.name, &job.scope, &answer.edits)
                    };
                    lines.extend(out.log);
                    suggestions = out.applied.into_iter().map(|applied| Suggestion { applied, state: SugState::Pending }).collect::<Vec<_>>();
                    if let Some(why) = answer.cut_off {
                        lines.push(edits::cut_off_line(answer.edits.len(), why));
                        answer_text = Some(reply.text.clone());
                    }
                    if suggestions.is_empty() && answer.edits.is_empty() && report.is_empty() {
                        lines.push(tl!("No changes suggested.").into());
                    }
                    log::info!("assistant: {} suggestions", suggestions.len());
                    if !suggestions.is_empty() {
                        app.ui.assistant.remember_name(&job.name);
                    }
                    Status::Done
                }
                Err(e) => {
                    log::warn!("assistant: answer not usable ({e:?}, stop reason {:?}, {} characters)", reply.stop_reason, reply.text.len());
                    lines.push(e.message());
                    if !matches!(e, edits::AnswerError::Empty { .. }) {
                        answer_text = Some(reply.text.clone());
                    }
                    Status::Failed
                }
            }
        }
        Err(AssistantError::Stopped) => Status::Stopped,
        Err(e) => {
            // Never holds the key (wordcraft_assistant::AssistantError).
            log::warn!("assistant: {e}");
            lines.push(capitalise(&e.to_string()));
            Status::Failed
        }
    };
    let rev = app.session.rev();
    if let Some(e) = entry_mut(app, job.doc, job.entry) {
        e.status = status;
        e.reply_time = now_hm();
        if !report.is_empty() {
            e.report = report;
        } else if status != Status::Done {
            e.report.clear();
        }
        e.lines.extend(lines);
        e.suggestions = suggestions;
        e.answer = answer_text.map(|t| t.chars().take(edits::MAX_ANSWER_CHARS).collect());
    }
    app.assistant_panel.checked = (job.doc, rev);
}

/// Mark suggestions accepted, rejected or edited away elsewhere (Review, undo) as gone.
fn refresh_states(app: &mut WordApp) {
    let key = (app.session.document_id(), app.session.rev());
    if app.assistant_panel.checked == key {
        return;
    }
    app.assistant_panel.checked = key;
    let doc = key.0;
    let Some(log) = app.assistant_panel.logs.get(&doc) else { return };
    let authors: Vec<String> = log.iter().filter(|e| e.suggestions.iter().any(|s| s.state == SugState::Pending)).map(|e| e.author.clone()).collect();
    if authors.is_empty() {
        return;
    }
    let mut live: HashMap<String, Vec<(String, String, Pos, Pos)>> = HashMap::new();
    for a in authors {
        if let std::collections::hash_map::Entry::Vacant(slot) = live.entry(a.clone()) {
            slot.insert(crate::review_panel::changes_by(app, &a));
        }
    }
    let comments = app.session.doc.comments.clone();
    let Some(log) = app.assistant_panel.logs.get_mut(&doc) else { return };
    for e in log.iter_mut() {
        let list = live.get(&e.author).cloned().unwrap_or_default();
        for s in e.suggestions.iter_mut().filter(|s| s.state == SugState::Pending) {
            let here = match s.applied.kind {
                EditKind::Comment => s.applied.comment.and_then(|id| comments.get(&id)).is_some_and(|c| !c.resolved),
                _ => find_group(&list, &parts_of(&s.applied)).is_some(),
            };
            if !here {
                s.state = SugState::Gone;
            }
        }
    }
}

/// The tracked changes an applied edit was made of (one per word-level piece).
pub(crate) fn parts_of(a: &Applied) -> Vec<(String, String)> {
    if a.parts.is_empty() { vec![(a.deleted.clone(), a.inserted.clone())] } else { a.parts.clone() }
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

/// The pending suggestions of the current document made of several pieces: (author, parts,
/// what the row shows deleted, inserted). Review shows each as one row.
pub(crate) fn pending_groups(app: &WordApp) -> Vec<(String, Vec<(String, String)>, String, String)> {
    let doc = app.session.document_id();
    app.assistant_panel
        .log(doc)
        .iter()
        .flat_map(|e| {
            e.suggestions
                .iter()
                .filter(|s| s.state == SugState::Pending && s.applied.parts.len() > 1)
                .map(|s| (e.author.clone(), s.applied.parts.clone(), s.applied.deleted.clone(), s.applied.inserted.clone()))
        })
        .collect()
}

/// Accept or reject suggestion `index` of entry `entry` in the current document (the person's
/// action, like Review's buttons).
pub fn resolve(app: &mut WordApp, entry: usize, index: usize, accept: bool) -> Result<Value, String> {
    let doc = app.session.document_id();
    let Some(e) = app.assistant_panel.log(doc).get(entry) else { return Err("no such entry".into()) };
    let author = e.author.clone();
    let Some(s) = e.suggestions.get(index).cloned() else { return Err("no such suggestion".into()) };
    if s.state != SugState::Pending {
        return Err(tl!("That suggestion was already dealt with.").into());
    }
    match s.applied.kind {
        EditKind::Comment => {
            let id = s.applied.comment.ok_or("that comment is gone")?;
            if accept {
                app.run("review.resolveComment", json!({"id": id, "value": true}))?;
            } else {
                app.run("review.deleteComment", json!({"id": id}))?;
            }
        }
        _ => {
            // All the edit's word-level pieces together: from the first one's start to the last
            // one's end (the words between them carry no change).
            let list = crate::review_panel::changes_by(app, &author);
            let found = find_group(&list, &parts_of(&s.applied)).and_then(|(i, j)| Some((list.get(i)?.2.clone(), list.get(j)?.3.clone())));
            let Some((a, b)) = found else {
                if let Some(x) = entry_mut(app, doc, entry).and_then(|e| e.suggestions.get_mut(index)) {
                    x.state = SugState::Gone;
                }
                return Err(tl!("That suggestion is no longer in the document.").into());
            };
            crate::review_panel::resolve_range(app, &a, &b, accept)?;
        }
    }
    if let Some(x) = entry_mut(app, doc, entry).and_then(|e| e.suggestions.get_mut(index)) {
        x.state = if accept { SugState::Accepted } else { SugState::Rejected };
    }
    app.assistant_panel.checked = (doc, app.session.rev());
    Ok(json!({"accepted": accept}))
}

/// Where the assistant is on the page while it works (A5): the caret, and the block around it.
pub fn presence(app: &WordApp) -> Option<(Pos, Pos, Pos)> {
    let job = app.assistant_panel.job.as_ref()?;
    if job.doc != app.session.document_id() {
        return None;
    }
    let caret = app.session.doc.clamp(job.presence.as_ref()?);
    let len = app.session.doc.para_at(&caret).map_or(0, |p| p.len());
    let a = Pos { off: 0, ..caret.clone() };
    let b = Pos { off: len, ..caret.clone() };
    Some((caret, a, b))
}

fn entry_json(e: &Entry) -> Value {
    let status = match e.status {
        Status::Working => "working",
        Status::Done => "done",
        Status::Stopped => "stopped",
        Status::Failed => "failed",
    };
    let sug: Vec<Value> = e
        .suggestions
        .iter()
        .map(|s| {
            let state = match s.state {
                SugState::Pending => "pending",
                SugState::Accepted => "accepted",
                SugState::Rejected => "rejected",
                SugState::Gone => "gone",
            };
            json!({"kind": s.applied.kind.id(), "deleted": s.applied.deleted, "inserted": s.applied.inserted, "quote": s.applied.quote, "note": s.applied.note, "state": state})
        })
        .collect();
    json!({"time": e.time, "instruction": e.instruction, "scope": e.scope, "author": e.author, "replyTime": e.reply_time, "status": status, "report": e.report, "lines": e.lines, "suggestions": sug, "answer": e.answer})
}

/// `ui.assistant.ask` / `.stop` / `.panel` / `.log` / `.suggestion`; `None` for other ids.
/// All are the person's: outside assistants are refused every `ui.assistant.*`
/// (`wordcraft_engine::assistant::denied`).
///
/// * `ui.assistant.ask` `{"instruction": string, "scope"?: "selection" | "section" | "document"}`
///   (default: the selection when there is one, else the whole document);
/// * `ui.assistant.stop`;
/// * `ui.assistant.panel` `{"value"?: bool}` (toggles without a value);
/// * `ui.assistant.log`: this document's work log and whether it is working;
/// * `ui.assistant.suggestion` `{"entry": n, "index": n, "accept": bool}`.
pub fn command(app: &mut WordApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    Some(match id {
        "ui.assistant.ask" => {
            let instruction = p.get("instruction").and_then(Value::as_str).unwrap_or_default().to_string();
            let kind = match p.get("scope") {
                None | Some(Value::Null) => {
                    if app.session.sel.is_collapsed() {
                        ScopeKind::Section
                    } else {
                        ScopeKind::Selection
                    }
                }
                Some(v) => match v.as_str().and_then(ScopeKind::from_id) {
                    Some(k) => k,
                    None => return Some(Err("`scope` must be selection, section or document".into())),
                },
            };
            match p.get("file") {
                None | Some(Value::Null) => ask(app, &instruction, kind),
                Some(Value::String(path)) => {
                    let instruction = if instruction.trim().is_empty() { FACT_CHECK.to_string() } else { instruction };
                    ask_file(app, &instruction, kind, path)
                }
                Some(_) => Err("`file` must be a path".into()),
            }
        }
        "ui.assistant.facts" => {
            // The quick ask: list the files in the allowed folders.
            let kind = if app.session.sel.is_collapsed() { ScopeKind::Section } else { ScopeKind::Selection };
            open_file_picker(app, kind).map(|n| json!({"files": n}))
        }
        "ui.assistant.stop" => Ok(json!({"stopped": stop(app)})),
        "ui.assistant.panel" => {
            let on = match p.get("value") {
                None | Some(Value::Null) => !app.assistant_panel.open,
                Some(v) => match v.as_bool() {
                    Some(b) => b,
                    None => return Some(Err("`value` must be true or false".into())),
                },
            };
            set_open(app, on);
            Ok(json!({"open": app.assistant_panel.open, "shown": visible(app)}))
        }
        "ui.assistant.log" => {
            poll(app);
            let doc = app.session.document_id();
            let list: Vec<Value> = app.assistant_panel.log(doc).iter().map(entry_json).collect();
            Ok(json!({"working": app.assistant_panel.working(), "entries": list}))
        }
        "ui.assistant.suggestion" => {
            let n = |k: &str| p.get(k).and_then(Value::as_u64).and_then(|v| usize::try_from(v).ok());
            let (Some(entry), Some(index), Some(accept)) = (n("entry"), n("index"), p.get("accept").and_then(Value::as_bool)) else {
                return Some(Err("give `entry`, `index` and `accept`".into()));
            };
            resolve(app, entry, index, accept)
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Drawing

/// "Check facts against a file…": the files in the allowed folders, one click to ask. Nothing
/// outside those folders can be chosen: only the listed files are offered, and the ask checks
/// the path again.
pub fn show_file_picker(app: &mut WordApp, ctx: &egui::Context) {
    let Some(p) = app.assistant_panel.picker.take() else { return };
    let t = Tokens::get(ctx);
    let m = MarginTokens::get(ctx);
    let folders = app.ui.assistant.allowed_folders();
    let mut open = true;
    let mut chosen: Option<std::path::PathBuf> = None;
    egui::Window::new(tl!("Check facts against a file"))
        .id(egui::Id::new("assistant_file_picker"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -20.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_width(480.0);
            mc::hint(ui, tl!("Choose a file from the folders you allowed. It is only read, never changed, and the assistant answers with comments on claims the file doesn’t support."));
            ui.add_space(8.0);
            egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                for f in &p.files {
                    let shown = shown_path(f, &folders);
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 4.0, t.hover);
                    }
                    let clip = ui.painter().with_clip_rect(r.shrink2(vec2(8.0, 0.0)));
                    clip.text(pos2(r.min.x + 8.0, r.center().y), egui::Align2::LEFT_CENTER, shown, mc::font(), t.text);
                    if resp.clicked() {
                        chosen = Some(f.clone());
                    }
                }
            });
        });
    let _ = &m;
    if let Some(f) = chosen {
        let (instruction, kind) = (p.instruction.clone(), p.scope);
        match ask_file(app, &instruction, kind, &f.to_string_lossy()) {
            Ok(_) => {}
            Err(e) => app.status(e),
        }
    } else if open {
        app.assistant_panel.picker = Some(p);
    }
}

/// A listed file as the person knows it: below its allowed folder, with the folder's name first.
fn shown_path(f: &std::path::Path, folders: &[String]) -> String {
    for folder in folders {
        let base = std::path::Path::new(folder);
        let canon = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
        if let Ok(rest) = f.strip_prefix(&canon) {
            let top = canon.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return format!("{top}/{}", rest.to_string_lossy());
        }
    }
    f.to_string_lossy().into_owned()
}

/// The panel, docked on the right (320 wide).
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::right("margin_assistant")
        .exact_size(crate::review_panel::PANEL_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| content(app, ui));
}

fn content(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let m = MarginTokens::get(ui.ctx());
    let name = crate::review_panel::shown_assistant_name();
    header(app, ui, &name, &t, &m);
    egui::Panel::bottom("margin_assistant_composer")
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(t.panel)
                .inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: 12 })
                .stroke(Stroke::new(1.0, t.border)),
        )
        .show(ui, |ui| composer(app, ui, &name, &t, &m));
    let doc = app.session.document_id();
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 16, top: 16, bottom: 16 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
            let n = app.assistant_panel.log(doc).len();
            if n == 0 {
                mc::hint(
                    ui,
                    &crate::i18n::fmt(
                        tl!("Ask {name} to shorten, fix or check part of this document. Every edit arrives as a suggestion you accept or reject."),
                        &[("name", &name)],
                    ),
                );
            }
            for i in 0..n {
                if i > 0 {
                    ui.add_space(16.0);
                }
                entry_ui(app, ui, doc, i, &t, &m);
            }
        });
    });
}

/// A1: dotted mark + "<name> (co-writer)", what it is doing, and Stop while it works.
fn header(app: &mut WordApp, ui: &mut Ui, name: &str, t: &Tokens, m: &MarginTokens) {
    let working = app.assistant_panel.working();
    let doing = match &app.assistant_panel.job {
        Some(job) => {
            let mut s = crate::i18n::fmt(tl!("Working in {scope}"), &[("scope", &scope_label(&job.scope))]);
            if job.quotes > 0 {
                s.push_str(" · ");
                s.push_str(&crate::i18n::fmt(
                    if job.quotes == 1 { tl!("{n} edit so far") } else { tl!("{n} edits so far") },
                    &[("n", &job.quotes.to_string())],
                ));
            }
            s
        }
        None => tl!("Suggests edits; you decide").to_string(),
    };
    let mut stop_clicked = false;
    egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: 12 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(4.0, 40.0), Sense::hover());
            mark_line(ui.painter(), pos2(r.center().x, r.min.y + 4.0), pos2(r.center().x, r.max.y - 4.0), m.assistant, true);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let title = crate::i18n::fmt(tl!("{name} (co-writer)"), &[("name", name)]);
                ui.label(egui::RichText::new(title).font(mc::font()).color(t.text));
                let w = (ui.available_width() - if working { 90.0 } else { 0.0 }).max(60.0);
                ui.set_max_width(w);
                ui.add(egui::Label::new(egui::RichText::new(doing).font(mc::font()).color(t.text_dim)).truncate());
            });
            if working {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if mc::button(ui, "Stop", false, true).clicked() {
                        stop_clicked = true;
                    }
                });
            }
        });
    });
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.min.y, Stroke::new(1.0, t.border));
    if stop_clicked {
        stop(app);
    }
}

/// A2 + A3: "You · time" and the instruction; the reply under a dotted rule with its report and
/// the suggestions group.
fn entry_ui(app: &mut WordApp, ui: &mut Ui, doc: u64, i: usize, t: &Tokens, m: &MarginTokens) {
    let Some(e) = app.assistant_panel.log(doc).get(i).cloned() else { return };
    ui.label(egui::RichText::new(format!("{} · {} · {}", tl!("You"), e.time, e.scope)).font(mc::font()).color(t.text_dim));
    ui.add(egui::Label::new(egui::RichText::new(&e.instruction).font(mc::font()).color(t.text)).wrap());
    ui.add_space(8.0);
    // The reply, indented under a 2 pt dotted rule in `assistant`.
    let top = ui.cursor().min;
    let mut clicked: Option<(usize, bool)> = None;
    let mut show_review = false;
    let mut toggle_answer = false;
    let inner = ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            let when = if e.status == Status::Working { tl!("now").to_string() } else { e.reply_time.clone() };
            ui.label(
                egui::RichText::new(format!("{} · {when}", wordcraft_engine::authorship::display_name(&e.author))).font(mc::font()).color(t.text_dim),
            );
            let report = match (e.status, e.report.trim().is_empty()) {
                (Status::Working, true) => tl!("Reading…").to_string(),
                (_, false) => e.report.clone(),
                (Status::Stopped, true) => String::new(),
                (_, true) => String::new(),
            };
            if !report.is_empty() {
                ui.add(egui::Label::new(egui::RichText::new(report).font(mc::font()).color(t.text)).wrap());
            }
            for l in &e.lines {
                ui.add(egui::Label::new(egui::RichText::new(l).font(mc::font()).color(t.text_dim)).wrap());
            }
            if let Some(raw) = &e.answer {
                if mc::link(ui, if e.show_answer { "Hide answer" } else { "Show answer" }).clicked() {
                    toggle_answer = true;
                }
                if e.show_answer {
                    egui::Frame::NONE.stroke(Stroke::new(1.0, t.border)).corner_radius(6).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt(("assistant_answer", i)).max_height(220.0).show(ui, |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(raw.as_str()).monospace().color(t.text)).wrap().selectable(true));
                        });
                    });
                }
            }
            if !e.suggestions.is_empty() {
                ui.add_space(4.0);
                clicked = suggestions_group(ui, &e, t, m);
                if mc::link(ui, "Show in Review").clicked() {
                    show_review = true;
                }
            }
        });
    });
    let bottom = inner.response.rect.max.y;
    mark_line(ui.painter(), pos2(top.x + 1.0, top.y + 2.0), pos2(top.x + 1.0, bottom - 2.0), m.assistant, true);
    if let Some((k, accept)) = clicked
        && let Err(err) = resolve(app, i, k, accept)
    {
        app.status(err);
    }
    if toggle_answer && let Some(x) = entry_mut(app, doc, i) {
        x.show_answer = !x.show_answer;
    }
    if show_review {
        app.session.view.comments_pane = true;
        app.ui.margin.inspector = false;
    }
}

/// A3: a bordered group (radius 8) of 36 pt rows, each with accept / reject. The row index and
/// the choice clicked.
fn suggestions_group(ui: &mut Ui, e: &Entry, t: &Tokens, m: &MarginTokens) -> Option<(usize, bool)> {
    let mut out = None;
    egui::Frame::NONE.stroke(Stroke::new(1.0, t.border)).corner_radius(8).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let n = e.suggestions.len();
        for (k, s) in e.suggestions.iter().enumerate() {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(vec2(w, SUGGESTION_H), Sense::hover());
            if k + 1 < n {
                ui.painter().hline(r.x_range(), r.max.y, Stroke::new(1.0, t.border));
            }
            let mut row =
                ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let pending = s.state == SugState::Pending;
            let buttons_w = if pending { 56.0 } else { 80.0 };
            let text_w = (row.available_width() - buttons_w - 6.0).max(40.0);
            let a = &s.applied;
            let mut parts: Vec<(String, Deco)> = Vec::new();
            match a.kind {
                EditKind::Comment => {
                    parts.push((format!("“{}” ", crate::review_panel::clip(&a.quote, 14)), Deco::None));
                    parts.push((crate::review_panel::clip(&a.note, 40), Deco::Underline));
                }
                _ => {
                    if !a.deleted.is_empty() {
                        parts.push((crate::review_panel::clip(&a.deleted, 14), Deco::Strike));
                    }
                    if !a.deleted.is_empty() && !a.inserted.is_empty() {
                        parts.push((" → ".into(), Deco::None));
                    }
                    if !a.inserted.is_empty() {
                        parts.push((crate::review_panel::clip(&a.inserted, 14), Deco::Underline));
                    }
                }
            }
            row.allocate_ui_with_layout(vec2(text_w, SUGGESTION_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(text_w);
                single_line_excerpt(ui, &parts, m.assistant, t, a.kind == EditKind::Comment);
            });
            row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pending {
                    if crate::review_panel::icon_button(ui, "reject", "Reject", t) {
                        out = Some((k, false));
                    }
                    if crate::review_panel::icon_button(ui, "accept", "Accept", t) {
                        out = Some((k, true));
                    }
                } else {
                    let label = match s.state {
                        SugState::Accepted => tl!("Accepted"),
                        SugState::Rejected => tl!("Rejected"),
                        _ => tl!("Resolved"),
                    };
                    ui.label(egui::RichText::new(label).font(mc::font()).color(t.text_dim));
                }
            });
        }
    });
    out
}

/// One line of "deleted → inserted" (struck / dotted underline), cut to the width.
fn single_line_excerpt(ui: &mut Ui, parts: &[(String, Deco)], color: Color32, t: &Tokens, comment: bool) {
    if comment {
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        crate::icons::paint(ui.painter(), r, "comment", color, color);
        ui.add_space(4.0);
    }
    let mut job = egui::text::LayoutJob { wrap: egui::text::TextWrapping::truncate_at_width(ui.available_width()), ..Default::default() };
    let mut spans = Vec::new();
    for (text, deco) in parts {
        let start = job.text.len();
        let c = if *deco == Deco::None { t.text_dim } else { color };
        job.append(text, 0.0, egui::TextFormat { font_id: mc::font(), color: c, ..Default::default() });
        spans.push((start..job.text.len(), *deco));
    }
    let galley = ui.painter().layout_job(job);
    let (r, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(r.min, galley.clone(), t.text);
    // Strike and dotted underline under each part's glyphs (the first row only: one line).
    let Some(row) = galley.rows.first() else { return };
    let mut byte = 0usize;
    let mut extents: Vec<(Deco, f32, f32)> = Vec::new();
    for g in &row.glyphs {
        let deco = spans.iter().find(|(s, _)| s.contains(&byte)).map_or(Deco::None, |(_, d)| *d);
        byte += g.chr.len_utf8();
        let (x0, x1) = (r.min.x + row.pos.x + g.pos.x, r.min.x + row.pos.x + g.pos.x + g.advance_width);
        match extents.last_mut() {
            Some((d, _, e)) if *d == deco => *e = x1,
            _ => extents.push((deco, x0, x1)),
        }
    }
    for (d, x0, x1) in extents {
        let y = match d {
            Deco::Strike => r.min.y + row.pos.y + row.size.y * 0.55,
            Deco::Underline => r.min.y + row.pos.y + row.size.y - 1.0,
            Deco::None => continue,
        };
        if d == Deco::Strike {
            ui.painter().line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, color));
        } else {
            let len = x1 - x0;
            let n = ((len / 3.0).floor() as usize).min(400);
            for k in 0..=n {
                ui.painter().circle_filled(pos2(x0 + len * k as f32 / n.max(1) as f32, y), 0.7, color);
            }
        }
    }
}

/// A4: Scope popup, the 76 pt box with the send button, and the lock line.
fn composer(app: &mut WordApp, ui: &mut Ui, name: &str, t: &Tokens, m: &MarginTokens) {
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
    let has_sel = !app.session.sel.is_collapsed();
    if app.assistant_panel.scope == ScopeKind::Selection && !has_sel && !app.assistant_panel.working() {
        app.assistant_panel.scope = ScopeKind::Section;
    }
    mc::labelled_row(ui, "Scope", |ui| {
        let w = ui.available_width();
        let cur = tl!(app.assistant_panel.scope.label()).to_string();
        let mut chosen = None;
        mc::popup_button_with(ui, egui::Id::new("margin_assistant_scope"), w, &cur, |ui| {
            for k in ScopeKind::ALL {
                let enabled = k != ScopeKind::Selection || has_sel;
                let r = ui.add_enabled_ui(enabled, |ui| mc::menu_row(ui, tl!(k.label()), k == app.assistant_panel.scope)).inner;
                if r.clicked() && enabled {
                    chosen = Some(k);
                    ui.close();
                }
            }
        });
        if let Some(k) = chosen {
            app.assistant_panel.scope = k;
        }
    });
    // The box: 76 pt, the send button in its bottom-right corner.
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, COMPOSER_H), Sense::hover());
    let edit_id = egui::Id::new("margin_assistant_draft");
    let focused = ui.ctx().memory(|mem| mem.has_focus(edit_id));
    ui.painter().rect(r, mc::RADIUS, m.field_bg, Stroke::new(1.0, if focused { t.accent } else { t.input_border }), egui::StrokeKind::Inside);
    // ↩ sends, ⇧↩ is a new line.
    let enter = focused && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let edit_rect = Rect::from_min_max(r.min + vec2(8.0, 6.0), r.max - vec2(40.0, 6.0));
    let hint = crate::i18n::fmt(tl!("Ask {name} to…"), &[("name", name)]);
    ui.put(
        edit_rect,
        egui::TextEdit::multiline(&mut app.assistant_panel.draft)
            .id(edit_id)
            .frame(egui::Frame::NONE)
            .font(mc::font())
            .text_color(t.text)
            .hint_text(egui::RichText::new(hint).color(t.text_dim).font(mc::font()))
            .desired_width(edit_rect.width()),
    );
    let can_send = !app.assistant_panel.draft.trim().is_empty() && !app.assistant_panel.working();
    let b = Rect::from_min_size(pos2(r.max.x - 34.0, r.max.y - 34.0), vec2(28.0, 28.0));
    let resp = ui.interact(b, egui::Id::new("margin_assistant_send"), if can_send { Sense::click() } else { Sense::hover() });
    let fill = if can_send { t.accent } else { t.accent.gamma_multiply(0.4) };
    ui.painter().rect_filled(b, mc::RADIUS, fill);
    crate::icons::paint(ui.painter(), Rect::from_center_size(b.center(), vec2(16.0, 16.0)), "send", t.on_accent, t.on_accent);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, can_send, tl!("Send")));
    if (resp.clicked() || enter) && can_send {
        let text = std::mem::take(&mut app.assistant_panel.draft);
        let kind = app.assistant_panel.scope;
        if let Err(e) = ask(app, &text, kind) {
            app.assistant_panel.draft = text;
            app.status(e);
        }
    }
    // The permanent lock line.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (lr, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
        crate::icons::paint(ui.painter(), Rect::from_center_size(lr.center(), vec2(14.0, 14.0)), "lock", t.text_dim, t.text_dim);
        ui.label(egui::RichText::new(tl!("Edits arrive as tracked suggestions")).font(mc::font()).color(t.text_dim));
    });
}

/// A5 on the page: a 2 pt caret in `assistant` with a name tag (11 pt, the one size exception),
/// `assistant_tint` over the block it works on, and a dotted bar in the margin beside it.
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
    // The name tag sits on the caret's top, 11 pt white on `assistant`.
    let name = crate::review_panel::shown_assistant_name();
    let g = painter.layout_no_wrap(name, crate::theme::regular(11.0), Color32::WHITE);
    let tag = Rect::from_min_size(pos2(x - 1.0, y0 - g.size().y - 4.0), g.size() + vec2(8.0, 4.0));
    painter.rect_filled(tag, 3.0, color);
    painter.galley(tag.min + vec2(4.0, 2.0), g, Color32::WHITE);
}

#[cfg(test)]
mod tests;
