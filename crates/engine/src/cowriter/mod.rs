//! O3 (OPTIMO.md §0, §4): the co-writer's verb runner and the guards that enforce the five
//! product rules in code.
//!
//! * **One runner.** A verb ([`Verb`]) is asked for a model prompt ([`prompt`]; `None` when it
//!   needs no model), its answer is read into [`Output`]s ([`run`]), and the outputs land in the
//!   document ([`apply`]) through `Session::run_as_assistant`, so everything the co-writer adds is
//!   a tracked change or a comment under the AI author (rule 3). The network is the caller's
//!   (`wordcraft_assistant`, from the UI's background job); nothing here talks to a provider.
//! * **Guards** ([`guard`], run by both [`run`] and [`apply`]): the outputs each verb may give
//!   ([`Verb::allows`], the §4 table), the new words each may add (none for Read as a reader,
//!   Check, Structure and Format for me; one sentence per selected sentence for Alternatives; one
//!   line for I'm stuck; heading text only for Structure; one sourced sentence for Research), and
//!   for Format for me the plain-text identity check ([`same_words`]): if the document's words
//!   would change, everything it did is taken back and the run says so (rule 5). Anything refused
//!   changes nothing and is reported.
//! * **Plug-in point.** Each verb lives in its own file here, with its prompt, how it reads the
//!   model's answer, `AVAILABLE` and the landing of the outputs only it gives. A later phase
//!   replaces its own file and flips `AVAILABLE`; this file, the guards and the ⌘Y list stay as
//!   they are. Options are a JSON object each verb reads its own keys from.
//!
//! These rules bind the in-app co-writer only. Outside assistants over MCP or the control channel
//! keep the access `crate::assistant` gives them.

mod alternatives;
mod ask;
mod check;
mod format;
mod read;
mod research;
mod structure;
mod stuck;
#[cfg(test)]
mod tests;

use serde_json::Value;

use crate::Session;
use crate::assistant_edits::{self as edits, Applied, Edit, EditKind, ScopeText};

/// The co-writer's verbs (OPTIMO §4), plus Ask: free text from ⌘Y, answered with notes only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Verb {
    ReadAsReader,
    CheckFacts,
    Alternatives,
    Structure,
    Research,
    Stuck,
    FormatForMe,
    Ask,
}

impl Verb {
    /// The verbs in ⌘Y's Co-writer group order (Y2). Ask is the ask row, not in the group.
    pub const GROUP: [Verb; 7] =
        [Verb::Alternatives, Verb::ReadAsReader, Verb::Structure, Verb::Research, Verb::CheckFacts, Verb::Stuck, Verb::FormatForMe];

    pub fn id(self) -> &'static str {
        match self {
            Verb::ReadAsReader => "read",
            Verb::CheckFacts => "check",
            Verb::Alternatives => "alternatives",
            Verb::Structure => "structure",
            Verb::Research => "research",
            Verb::Stuck => "stuck",
            Verb::FormatForMe => "format",
            Verb::Ask => "ask",
        }
    }

    pub fn from_id(id: &str) -> Option<Verb> {
        Verb::GROUP.into_iter().chain([Verb::Ask]).find(|v| v.id() == id)
    }

    /// The name in ⌘Y (Y2).
    pub fn label(self) -> &'static str {
        match self {
            Verb::ReadAsReader => "Read as a reader…",
            Verb::CheckFacts => "Check facts and names",
            Verb::Alternatives => "Alternatives",
            Verb::Structure => "Structure",
            Verb::Research => "Research…",
            Verb::Stuck => "I’m stuck",
            Verb::FormatForMe => "Format for me",
            Verb::Ask => "Ask the co-writer",
        }
    }

    /// The one-line description in ⌘Y, in `text_dim`.
    pub fn description(self) -> &'static str {
        match self {
            Verb::ReadAsReader => "Notes where a reader would stumble",
            Verb::CheckFacts => "Numbers, dates and names against your files",
            Verb::Alternatives => "Other ways to say the selected sentence",
            Verb::Structure => "The shape of the document, and what is missing",
            Verb::Research => "Findings with sources, as footnotes or quotes",
            Verb::Stuck => "Questions to get you going",
            Verb::FormatForMe => "Styles, lists and typography; never your words",
            Verb::Ask => "Notes in the margin, never inserted text",
        }
    }

    /// The default shortcut, portable form (`Mod` = ⌘). OPTIMO's keys where they are free:
    /// ⌥⌘S is Split (Word's key) and ⌥⌘F Footnote, so Structure takes ⌃⌥⌘S and Research ⇧⌥⌘F;
    /// ⌥⌘R and ⌥⌘T are the Registered and Trademark signs outside macOS, so they are Mac-only.
    /// `tests::verb_shortcuts_are_free_in_the_registry` keeps them clear of every command.
    pub fn shortcut(self) -> &'static str {
        match self {
            Verb::Alternatives => "Mod+Alt+A",
            Verb::ReadAsReader => crate::cmd::mac_or("Mod+Alt+R", ""),
            Verb::Structure => crate::cmd::mac_or("Mod+Ctrl+Alt+S", ""),
            Verb::Research => "Mod+Alt+Shift+F",
            Verb::FormatForMe => crate::cmd::mac_or("Mod+Alt+T", ""),
            Verb::CheckFacts | Verb::Stuck | Verb::Ask => "",
        }
    }

    /// Whether the verb has landed (its phase flips it in its own file). ⌘Y lists only these.
    pub fn available(self) -> bool {
        match self {
            Verb::ReadAsReader => read::AVAILABLE,
            Verb::CheckFacts => check::AVAILABLE,
            Verb::Alternatives => alternatives::AVAILABLE,
            Verb::Structure => structure::AVAILABLE,
            Verb::Research => research::AVAILABLE,
            Verb::Stuck => stuck::AVAILABLE,
            Verb::FormatForMe => format::AVAILABLE,
            Verb::Ask => ask::AVAILABLE,
        }
    }

    /// The §4 table: the outputs the verb may give.
    pub fn allows(self, kind: OutputKind) -> bool {
        use OutputKind as K;
        match self {
            Verb::ReadAsReader | Verb::CheckFacts | Verb::Ask => kind == K::Comment,
            Verb::Alternatives => kind == K::Replacement,
            Verb::Structure => matches!(kind, K::HeadingMove | K::HeadingInsert | K::Comment),
            Verb::Research => matches!(kind, K::Footnote | K::Quote | K::Comment),
            Verb::Stuck => matches!(kind, K::Comment | K::FirstLine),
            Verb::FormatForMe => kind == K::FormatChange,
        }
    }
}

/// A source attached to a footnote or quote (rule 4): at least a title or an address.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    pub title: String,
    pub url: String,
    /// "site · year", as shown in `text_dim`.
    pub detail: String,
}

/// What a verb may give back. Anchors are exact quotes of the document inside the scope.
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// A margin note: "Check", "Question", "Strong line", "Note"…, with optional one-click options.
    Comment { anchor: String, kind: String, text: String, options: Vec<String> },
    /// Alternatives: the anchor (the selected sentences) said another way.
    Replacement { anchor: String, with: String },
    /// Structure: move the section under `heading` to just before the heading `before`.
    HeadingMove { heading: String, before: String },
    /// Structure: a heading (text only) after the paragraph quoted by `after`.
    HeadingInsert { after: String, text: String, level: u8 },
    /// Research: a footnote on `anchor` with its source.
    Footnote { anchor: String, text: String, source: Source },
    /// Research: a marked quote after the paragraph quoted by `after`, with its source.
    Quote { after: String, text: String, source: Source },
    /// I'm stuck: one rough first line at `at` (opt-in).
    FirstLine { at: String, text: String },
    /// Format for me: one formatting command (its own file lists which).
    FormatChange { command: String, params: Value },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    Comment,
    Replacement,
    HeadingMove,
    HeadingInsert,
    Footnote,
    Quote,
    FirstLine,
    FormatChange,
}

impl Output {
    pub fn kind(&self) -> OutputKind {
        match self {
            Output::Comment { .. } => OutputKind::Comment,
            Output::Replacement { .. } => OutputKind::Replacement,
            Output::HeadingMove { .. } => OutputKind::HeadingMove,
            Output::HeadingInsert { .. } => OutputKind::HeadingInsert,
            Output::Footnote { .. } => OutputKind::Footnote,
            Output::Quote { .. } => OutputKind::Quote,
            Output::FirstLine { .. } => OutputKind::FirstLine,
            Output::FormatChange { .. } => OutputKind::FormatChange,
        }
    }

    /// The text it is about, for report lines.
    fn about(&self) -> &str {
        match self {
            Output::Comment { anchor, .. } | Output::Replacement { anchor, .. } | Output::Footnote { anchor, .. } => anchor,
            Output::HeadingMove { heading, .. } => heading,
            Output::HeadingInsert { text, .. } | Output::Quote { text, .. } | Output::FirstLine { text, .. } => text,
            Output::FormatChange { command, .. } => command,
        }
    }
}

/// What to send to the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
}

/// One run's inputs, handed to the verb's file.
pub struct Call<'a> {
    pub session: &'a Session,
    pub scope: &'a ScopeText,
    /// The verb's own options (Ask: `{"instruction": "…"}`).
    pub options: &'a Value,
    /// The AI author name the outputs land under.
    pub name: &'a str,
}

/// The model's reply, as the caller received it.
#[derive(Clone, Copy, Debug)]
pub struct Reply<'a> {
    pub text: &'a str,
    pub stop_reason: Option<&'a str>,
}

/// What a run gave, after the guards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ran {
    /// The model's one-line summary (shown at the top of the margin).
    pub report: String,
    pub outputs: Vec<Output>,
    /// What was refused, skipped or cut off, one line each.
    pub lines: Vec<String>,
}

/// What [`apply`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Landed {
    /// Notes made: (comment id, kind).
    pub notes: Vec<(u32, String)>,
    /// Tracked changes and comments made, in document order.
    pub applied: Vec<Applied>,
    pub lines: Vec<String>,
    /// Format for me: (words kept, words changed) from the identity check.
    pub words: Option<(usize, usize)>,
}

/// Most outputs one run may give.
pub const MAX_OUTPUTS: usize = 50;
/// I'm stuck's rough first line, and a heading, in words.
pub const MAX_LINE_WORDS: usize = 40;
pub const MAX_HEADING_WORDS: usize = 12;
/// A footnote's or quote's sentence, in words.
pub const MAX_SOURCED_WORDS: usize = 60;

fn not_yet(verb: Verb) -> String {
    format!("{} arrives in a later version of Optimo.", verb.label().trim_end_matches('…'))
}

/// The prompt for `verb`, or `None` when it works without a model.
pub fn prompt(verb: Verb, c: &Call) -> Result<Option<Prompt>, String> {
    if !verb.available() {
        return Err(not_yet(verb));
    }
    match verb {
        Verb::ReadAsReader => read::prompt(c),
        Verb::CheckFacts => check::prompt(c),
        Verb::Alternatives => alternatives::prompt(c),
        Verb::Structure => structure::prompt(c),
        Verb::Research => research::prompt(c),
        Verb::Stuck => stuck::prompt(c),
        Verb::FormatForMe => format::prompt(c),
        Verb::Ask => ask::prompt(c),
    }
}

/// `cowriter::run(verb, scope, options)`: read the reply (`None` for a verb without a model) into
/// outputs, guarded. Nothing changes in the document yet.
pub fn run(verb: Verb, c: &Call, reply: Option<Reply>) -> Result<Ran, String> {
    if !verb.available() {
        return Err(not_yet(verb));
    }
    let mut ran = match verb {
        Verb::ReadAsReader => read::read(c, reply),
        Verb::CheckFacts => check::read(c, reply),
        Verb::Alternatives => alternatives::read(c, reply),
        Verb::Structure => structure::read(c, reply),
        Verb::Research => research::read(c, reply),
        Verb::Stuck => stuck::read(c, reply),
        Verb::FormatForMe => format::read(c, reply),
        Verb::Ask => ask::read(c, reply),
    }?;
    let (kept, refused) = guard(verb, &c.scope.text, std::mem::take(&mut ran.outputs));
    ran.outputs = kept;
    ran.lines.extend(refused);
    Ok(ran)
}

// ---------------------------------------------------------------------------------------------
// Guards

/// Sentences in `text`: runs holding a letter or digit, each ended by `.`, `!`, `?` or `…` before
/// a space (so "3.5" and "e.g.x" don't end one) or by the end of the text.
pub fn sentences(text: &str) -> usize {
    let mut n = 0usize;
    let mut open = false;
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if c.is_alphanumeric() {
            open = true;
        }
        let end = matches!(c, '.' | '!' | '?' | '…' | '。' | '！' | '？');
        let next_breaks = it.peek().is_none_or(|n| n.is_whitespace() || matches!(n, '"' | '\'' | '”' | '’' | ')' | '»'));
        if end && open && next_breaks {
            n += 1;
            open = false;
        }
    }
    n + usize::from(open)
}

fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn one_line(text: &str) -> bool {
    !text.chars().any(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}' | '\u{b}' | '\u{c}'))
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// Why `o` breaks a rule for `verb`, if it does. `scope` is the text the verb was given.
fn refusal(verb: Verb, o: &Output, scope: &str) -> Option<String> {
    if !verb.allows(o.kind()) {
        return Some(format!("{} can't give a {:?}: it only gives what OPTIMO's table allows", verb.label().trim_end_matches('…'), o.kind()));
    }
    let too_long = |s: &str, max: usize| s.chars().count() > max;
    match o {
        Output::Comment { anchor, text, options, .. } => {
            if blank(anchor) || blank(text) {
                return Some("a note needs a quote and a text".into());
            }
            if too_long(text, edits::MAX_NOTE_CHARS) || options.len() > 4 || options.iter().any(|x| blank(x) || too_long(x, 60)) {
                return Some("the note is too long".into());
            }
        }
        Output::Replacement { anchor, with } => {
            if blank(anchor) || blank(with) || !one_line(with) {
                return Some("an alternative is one line in place of the selected sentences".into());
            }
            if !scope.contains(anchor.as_str()) {
                return Some("an alternative may only reword the selected sentences".into());
            }
            // Rule 1: at most one new sentence per selected sentence, and no run-on padding.
            if sentences(with) > sentences(anchor).max(1) || words(with) > words(anchor).saturating_mul(2).saturating_add(8) {
                return Some("it would add sentences of its own (rule 1)".into());
            }
        }
        Output::HeadingMove { heading, before } => {
            if blank(heading) || blank(before) || heading == before {
                return Some("a move names two different headings".into());
            }
        }
        Output::HeadingInsert { after, text, level } => {
            if blank(after) || blank(text) || !one_line(text) || !(1..=6).contains(level) {
                return Some("a heading is one line, level 1–6".into());
            }
            if words(text) > MAX_HEADING_WORDS || sentences(text) > 1 {
                return Some("a heading is a heading, not prose (rule 1)".into());
            }
        }
        Output::Footnote { anchor: at, text, source } | Output::Quote { after: at, text, source } => {
            if blank(&source.title) && blank(&source.url) {
                return Some("it has no source (rule 4)".into());
            }
            if blank(at) || blank(text) || !one_line(text) || sentences(text) > 1 || words(text) > MAX_SOURCED_WORDS {
                return Some("a finding is one sourced sentence (rules 1 and 4)".into());
            }
        }
        Output::FirstLine { at, text } => {
            if blank(at) || blank(text) || !one_line(text) || sentences(text) > 1 || words(text) > MAX_LINE_WORDS {
                return Some("a first line is one line (rule 1)".into());
            }
        }
        Output::FormatChange { command, .. } => {
            if !format::COMMANDS.iter().any(|p| command == p || (p.ends_with('.') && command.starts_with(p))) {
                return Some(format!("{command} is not a formatting command (rule 5)"));
            }
        }
    }
    None
}

/// Split `outputs` into what `verb` may give and a line for each one refused. `scope` is the text
/// the verb was given (an alternative may only reword text in it).
pub fn guard(verb: Verb, scope: &str, outputs: Vec<Output>) -> (Vec<Output>, Vec<String>) {
    let mut kept = Vec::new();
    let mut lines = Vec::new();
    for o in outputs {
        if kept.len() >= MAX_OUTPUTS {
            lines.push(format!("Refused the rest: at most {MAX_OUTPUTS} at a time."));
            break;
        }
        match refusal(verb, &o, scope) {
            Some(why) => lines.push(format!("Refused “{}”: {why}.", short(o.about()))),
            None => kept.push(o),
        }
    }
    (kept, lines)
}

fn short(s: &str) -> String {
    let s = s.trim();
    let t: String = s.chars().take(40).collect();
    if s.chars().count() > 40 { format!("{t}…") } else { t }
}

/// A document's words for the identity check: typographic quotes, ellipses, dashes and spaces
/// made plain, list markers at line starts and all whitespace dropped, then split into words.
pub fn plain_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut s = String::with_capacity(line.len());
        for c in line.chars() {
            match c {
                '‘' | '’' | '‚' | '‛' | '′' | '`' | '´' => s.push('\''),
                '“' | '”' | '„' | '‟' | '″' | '«' | '»' | '‹' | '›' => s.push('"'),
                '–' | '—' | '‒' | '―' | '−' | '‐' | '‑' => s.push('-'),
                '…' => s.push_str("..."),
                '\u{ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}' => {}
                c if c.is_whitespace() => s.push(' '),
                c => s.push(c),
            }
        }
        let mut toks = s.split_whitespace().peekable();
        // A list marker the formatting turned into a real list: "-", "*", "•", "1.", "1)", "a)".
        if let Some(first) = toks.peek() {
            let marker = matches!(*first, "-" | "*" | "•" | "◦" | "▪" | "·")
                || first.strip_suffix(['.', ')']).is_some_and(|n| {
                    !n.is_empty()
                        && (n.chars().all(|c| c.is_ascii_digit()) && n.len() <= 3 || n.chars().count() == 1 && n.chars().all(char::is_alphabetic))
                });
            if marker {
                toks.next();
            }
        }
        out.extend(toks.map(str::to_string));
    }
    out
}

/// Rule 5: whether `before` and `after` hold the same words ([`plain_words`]); with (words kept,
/// words changed).
pub fn same_words(before: &str, after: &str) -> (bool, usize, usize) {
    let (a, b) = (plain_words(before), plain_words(after));
    if a == b {
        return (true, a.len(), 0);
    }
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count().min(a.len().min(b.len()) - head);
    let changed = a.len().max(b.len()) - head - tail;
    (false, head + tail, changed)
}

// ---------------------------------------------------------------------------------------------
// Landing

/// Land `outputs` in the document under `name`, inside `scope`, after the guards. Comments become
/// comments by the AI author; the other outputs are landed by the verb that gives them. Format
/// for me is taken back entirely when the words would change (rule 5). The person's selection
/// and panels are as they were.
pub fn apply(s: &mut Session, name: &str, verb: Verb, scope: &ScopeText, outputs: Vec<Output>) -> Landed {
    let mut out = Landed::default();
    let (outputs, refused) = guard(verb, &scope.text, outputs);
    out.lines.extend(refused);
    if outputs.is_empty() {
        return out;
    }
    let pane = s.view.comments_pane;
    // Comments together (one locate pass, last first); kinds come back by quote and text.
    let notes: Vec<(String, String, String)> = outputs
        .iter()
        .filter_map(|o| match o {
            Output::Comment { anchor, kind, text, .. } => Some((anchor.clone(), kind.clone(), text.clone())),
            _ => None,
        })
        .collect();
    if !notes.is_empty() {
        let list: Vec<Edit> =
            notes.iter().map(|(q, _, t)| Edit { kind: EditKind::Comment, quote: q.clone(), with: String::new(), note: t.clone() }).collect();
        let done = edits::apply(s, name, scope, &list);
        out.lines.extend(done.log);
        for a in done.applied {
            let kind = notes.iter().find(|(q, _, t)| a.quote.contains(q.as_str()) && *t == a.note).map_or("Note", |n| n.1.as_str());
            if let Some(id) = a.comment {
                out.notes.push((id, kind.to_string()));
            }
            out.applied.push(a);
        }
    }
    if verb == Verb::FormatForMe {
        let mut landed = Landed::default();
        let r = keeping_words(s, |s| land_rest(s, name, scope, &outputs, &mut landed));
        out.lines.extend(landed.lines);
        match r {
            Ok(kept) => {
                out.applied.extend(landed.applied);
                out.words = Some((kept, 0));
            }
            Err((kept, changed)) => {
                out.words = Some((kept, changed));
                out.lines.push(format!(
                    "Format for me stopped: it would have changed {changed} of your words, so nothing was changed (rule 5). Please report this."
                ));
            }
        }
    } else {
        land_rest(s, name, scope, &outputs, &mut out);
    }
    s.view.comments_pane = pane;
    out
}

/// The outputs other than comments, each landed by the verb that gives it.
fn land_rest(s: &mut Session, name: &str, scope: &ScopeText, outputs: &[Output], out: &mut Landed) {
    for o in outputs {
        let r = match o {
            Output::Comment { .. } => continue,
            Output::Replacement { .. } => alternatives::land(s, name, scope, o),
            Output::HeadingMove { .. } | Output::HeadingInsert { .. } => structure::land(s, name, scope, o),
            Output::Footnote { .. } | Output::Quote { .. } => research::land(s, name, scope, o),
            Output::FirstLine { .. } => stuck::land(s, name, scope, o),
            Output::FormatChange { .. } => format::land(s, name, scope, o),
        };
        match r {
            Ok(a) => out.applied.extend(a),
            Err(why) => out.lines.push(format!("Couldn't land “{}”: {why}.", short(o.about()))),
        }
    }
}

/// Rule 5 around `f`: run it, then compare the body's words before and after ([`same_words`]).
/// The same: `Ok(words kept)`. Not: everything `f` did is taken back (document, selection, undo)
/// and `Err((words kept, words changed))`. Only the body counts: a header with the title or page
/// numbers in the footer are formatting. The body is read as it reads with every tracked change
/// accepted ([`final_body`]): a tracked typographic swap holds the old and the new characters.
pub fn keeping_words(s: &mut Session, f: impl FnOnce(&mut Session)) -> Result<usize, (usize, usize)> {
    let snap = s.edit_snapshot();
    let before = final_body(&s.doc.body, 0);
    f(s);
    let (same, kept, changed) = same_words(&before, &final_body(&s.doc.body, 0));
    if same {
        Ok(kept)
    } else {
        s.restore(snap);
        Err((kept, changed))
    }
}

/// The body's text with tracked deletions dropped (insertions kept), one line per paragraph and
/// table row, cells joined by tabs: `Document::plain_text` as it reads after accepting.
fn final_body(blocks: &wordcraft_doc::Blocks, depth: usize) -> String {
    let mut out = String::new();
    if depth > 16 {
        return out;
    }
    for b in blocks {
        match &**b {
            wordcraft_doc::Block::Para(p) => {
                out.push_str(&p.final_text());
                out.push('\n');
            }
            wordcraft_doc::Block::Table(t) => {
                for row in &t.rows {
                    let cells: Vec<String> =
                        row.cells.iter().map(|c| final_body(&c.blocks, depth + 1).trim_end_matches('\n').replace('\n', " ")).collect();
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                }
            }
        }
    }
    out
}
