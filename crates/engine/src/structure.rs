//! O7 (OPTIMO.md §8): what Structure needs besides the model call (`cowriter/structure.rs` asks
//! and guards). The document's shape ([`outline`]: one section per heading, with its words), the
//! model's answer ([`parse`]: purposes, moves, missing headings, observations), and the two
//! changes it may make, each landing only when the person accepts it:
//!
//! * [`move_section`]: a heading and everything under it cut and pasted before another heading
//!   as one tracked **move** by the AI author (`w:moveFrom` / `w:moveTo`), one undo step. The
//!   words before and after must be the same words (in any order) or nothing changes.
//! * [`insert_heading`]: a heading, text only, as one tracked insert by the AI author.

use serde_json::{Value, json};
use wordcraft_doc::{Block, Pos, StoryRef};

use crate::Session;
use crate::assistant_edits::{self as edits, Applied, EditKind};

/// A section with fewer words than this is thin: Structure offers I'm stuck for it (S3).
pub const THIN_WORDS: usize = 10;
/// A purpose line, in characters.
pub const MAX_PURPOSE_CHARS: usize = 140;
/// Most moves, missing headings and observations one answer may give.
pub const MAX_MOVES: usize = 5;
pub const MAX_MISSING: usize = 4;
pub const MAX_NOTES: usize = 6;
/// The kind line over an observation.
pub const KIND: &str = "Structure";

/// One heading and what is under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// The heading's top-level body block.
    pub block: usize,
    /// Its own content ends at the next heading of any level…
    pub end: usize,
    /// …and its subsections at the next heading of the same or a higher level.
    pub range_end: usize,
    /// 0 = Heading 1.
    pub level: u8,
    pub heading: String,
    /// Words of its own content (not the heading, not its subsections).
    pub words: usize,
}

impl Section {
    pub fn empty(&self) -> bool {
        self.words == 0
    }
    pub fn thin(&self) -> bool {
        self.words < THIN_WORDS
    }
}

fn level_of(s: &Session, b: &Block) -> Option<u8> {
    match b {
        Block::Para(p) => s.doc.styles.resolve_para(&p.props).outline_level.filter(|l| *l < 9),
        Block::Table(_) => None,
    }
}

fn block_words(b: &Block, depth: usize) -> usize {
    match b {
        Block::Para(p) => p.final_text().split_whitespace().count(),
        Block::Table(t) if depth < 16 => {
            t.rows.iter().flat_map(|r| &r.cells).flat_map(|c| c.blocks.iter()).map(|b| block_words(b, depth + 1)).fold(0, usize::saturating_add)
        }
        Block::Table(_) => 0,
    }
}

fn heading_text(b: &Block) -> String {
    match b {
        Block::Para(p) => p.final_text().split_whitespace().collect::<Vec<_>>().join(" "),
        Block::Table(_) => String::new(),
    }
}

/// The document's shape: its headings (top-level body paragraphs with an outline level, not
/// deleted) in order. Text before the first heading belongs to no section.
pub fn outline(s: &Session) -> Vec<Section> {
    let body = &s.doc.body;
    let heads: Vec<(usize, u8, String)> = body
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let t = heading_text(b);
            (!t.is_empty()).then_some(())?;
            level_of(s, b).map(|l| (i, l, t))
        })
        .collect();
    heads
        .iter()
        .enumerate()
        .map(|(k, (i, l, t))| {
            let end = heads.get(k + 1).map_or(body.len(), |h| h.0);
            let range_end = heads.iter().skip(k + 1).find(|h| h.1 <= *l).map_or(body.len(), |h| h.0);
            let words = body.get(i + 1..end).unwrap_or_default().iter().map(|b| block_words(b, 0)).fold(0, usize::saturating_add);
            Section { block: *i, end, range_end, level: *l, heading: t.clone(), words }
        })
        .collect()
}

/// Words outside any section (before the first heading), and in all.
pub fn total_words(s: &Session) -> usize {
    s.doc.body.iter().map(|b| block_words(b, 0)).fold(0, usize::saturating_add)
}

/// A heading as compared: words, case and quote styles don't matter.
fn key(t: &str) -> String {
    edits_key(t.trim().trim_matches(['"', '“', '”', '„', '«', '»', '\'']))
}

fn edits_key(t: &str) -> String {
    crate::cowriter::plain_words(t).join(" ").to_lowercase()
}

/// The section whose heading reads `name`, when exactly one does.
pub fn find<'a>(list: &'a [Section], name: &str) -> Option<&'a Section> {
    let k = key(name);
    if k.is_empty() {
        return None;
    }
    let mut it = list.iter().filter(|x| key(&x.heading) == k);
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

// ---------------------------------------------------------------------------------------------
// The answer

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Move {
    pub heading: String,
    pub before: String,
    pub why: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Missing {
    pub heading: String,
    /// A heading or a paragraph's words; empty = after the last section.
    pub after: String,
    /// 1-based, when the model gave one.
    pub level: Option<u8>,
    pub why: String,
}

/// The model's answer, as sent (not yet checked against the document).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answer {
    pub report: String,
    /// (heading, one-line purpose).
    pub purposes: Vec<(String, String)>,
    pub moves: Vec<Move>,
    pub missing: Vec<Missing>,
    /// (quote, note).
    pub notes: Vec<(String, String)>,
}

/// The JSON object in a reply: the whole text, or from its first `{` to its last `}`, without a
/// reasoning model's `<think>` block.
fn json_of(text: &str) -> Option<Value> {
    let mut t = text.trim();
    if let Some(rest) = t.strip_prefix("<think>") {
        t = rest.split_once("</think>")?.1.trim();
    }
    serde_json::from_str::<Value>(t)
        .ok()
        .or_else(|| t.find('{').zip(t.rfind('}')).and_then(|(a, b)| t.get(a..=b)).and_then(|j| serde_json::from_str(j).ok()))
        .filter(Value::is_object)
}

fn one_line(v: Option<&Value>, max: usize) -> String {
    let t = v.and_then(Value::as_str).unwrap_or_default();
    t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect()
}

fn list<'a>(v: &'a Value, k: &str, max: usize) -> impl Iterator<Item = &'a Value> {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default().iter().take(max)
}

/// Read the model's answer. Anything that isn't one JSON object is an error (nothing changes).
pub fn parse(text: &str) -> Result<Answer, String> {
    if text.len() > edits::MAX_ANSWER_CHARS {
        return Err("The answer was too long to read, so nothing was suggested.".into());
    }
    let Some(v) = json_of(text) else {
        return Err("The answer wasn't plain JSON, so nothing was suggested. Try again.".into());
    };
    let s = |o: &Value, k: &str| one_line(o.get(k), 400);
    Ok(Answer {
        report: one_line(v.get("report"), edits::MAX_REPORT_CHARS),
        purposes: list(&v, "purposes", 200)
            .map(|o| (s(o, "heading"), one_line(o.get("purpose"), MAX_PURPOSE_CHARS)))
            .filter(|(h, p)| !h.is_empty() && !p.is_empty())
            .collect(),
        moves: list(&v, "moves", 50).map(|o| Move { heading: s(o, "heading"), before: s(o, "before"), why: s(o, "why") }).collect(),
        missing: list(&v, "missing", 50)
            .map(|o| Missing {
                heading: o.get("heading").and_then(Value::as_str).unwrap_or_default().trim().to_string(),
                after: s(o, "after"),
                level: o.get("level").and_then(Value::as_u64).and_then(|l| u8::try_from(l).ok()).filter(|l| (1..=6).contains(l)),
                why: s(o, "why"),
            })
            .collect(),
        notes: list(&v, "notes", 50)
            .map(|o| (o.get("quote").and_then(Value::as_str).unwrap_or_default().to_string(), one_line(o.get("note"), edits::MAX_NOTE_CHARS)))
            .filter(|(q, n)| !q.trim().is_empty() && !n.is_empty())
            .collect(),
    })
}

/// Why `text` isn't a heading, if it isn't: one line, at most twelve words, not a sentence (no
/// full stop, exclamation mark or colon at the end; a question is fine).
pub fn not_a_heading(text: &str) -> Option<&'static str> {
    let t = text.trim();
    if t.is_empty() || t.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return Some("a heading is one line");
    }
    if t.split_whitespace().count() > crate::cowriter::MAX_HEADING_WORDS {
        return Some("a heading is a heading, not prose (rule 1)");
    }
    let end = t.trim_end_matches(['"', '\'', '”', '’', ')', '»']);
    if end.ends_with(['.', '!', ':', ';', ',']) && !end.ends_with("...") {
        return Some("it reads as a sentence, not a heading (rule 1)");
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Landing

/// Every word of the body as it reads with every tracked change accepted, sorted: a move keeps
/// them all, in another order.
fn word_bag(s: &Session) -> Vec<String> {
    let mut w: Vec<String> = s
        .doc
        .para_paths(StoryRef::Body)
        .iter()
        .filter_map(|p| s.doc.para(StoryRef::Body, p))
        .flat_map(|p| crate::cowriter::plain_words(&p.final_text()))
        .collect();
    w.sort_unstable();
    w
}

/// Run `f` (several commands) as one undo step that keeps every word and adds only the words of
/// `added`; anything else is taken back entirely (document, selection, undo, clipboard) with the
/// reason.
fn keeping_all(s: &mut Session, added: &str, f: impl FnOnce(&mut Session) -> Result<(), String>) -> Result<(), String> {
    let snap = s.edit_snapshot();
    let clip = (s.clipboard.clone(), s.clipboard_text.clone(), s.clip_history.clone());
    let mut before = word_bag(s);
    before.extend(crate::cowriter::plain_words(added));
    before.sort_unstable();
    let r = f(s);
    (s.clipboard, s.clipboard_text, s.clip_history) = clip;
    s.pending_move = None;
    let r = r.and_then(|_| if word_bag(s) == before { Ok(()) } else { Err("it would have lost or added words, so nothing was changed".into()) });
    if r.is_err() {
        s.restore(snap);
    }
    r
}

/// Run `id` as the AI author `name`, or (`None`) as the person with Track Changes on for it.
fn cmd(s: &mut Session, name: Option<&str>, id: &str, v: Value) -> Result<(), String> {
    let r = match name {
        Some(n) => s.run_as_assistant(n, id, &v),
        None => {
            let track = std::mem::replace(&mut s.doc.settings.track_changes, true);
            let r = s.run(id, &v);
            s.doc.settings.track_changes = track;
            r
        }
    };
    r.map(|_| ()).map_err(|e| e.to_string())
}

fn para_len(s: &Session, block: usize) -> usize {
    s.doc.body.get(block).and_then(|b| b.as_para()).map_or(0, |p| p.text.len())
}

/// Move the section under `heading` (with its subsections) to just before the heading `before`,
/// as one tracked move and one undo step: by the AI author `name`, or (`None`, the Shape
/// margin's drag) by the person. The caret ends on the moved heading.
pub fn move_section(s: &mut Session, name: Option<&str>, heading: &str, before: &str) -> Result<Applied, String> {
    let list = outline(s);
    let a = find(&list, heading).ok_or("that heading isn't in the document any more")?.clone();
    let b = find(&list, before).ok_or("the heading it goes before isn't in the document any more")?.clone();
    if (a.block..a.range_end).contains(&b.block) {
        return Err("a section can't go inside itself".into());
    }
    if a.range_end == b.block {
        return Err("it is already there".into());
    }
    let pane = s.view.comments_pane;
    let r = keeping_all(s, "", |s| {
        // The cut holds whole paragraphs with their marks. The last section has no paragraph
        // after it, so a tracked empty one goes there first (a range that starts with a mark
        // doesn't reject cleanly); once accepted, the document ends in that empty paragraph.
        let mut n = s.doc.body.len();
        if a.range_end >= n {
            let last = n.saturating_sub(1);
            s.sel = crate::Selection::caret(Pos::body(last, para_len(s, last)));
            cmd(s, name, "text.newParagraph", json!({}))?;
            s.join_next_undo();
            n = s.doc.body.len();
        }
        let to = a.range_end.min(n.saturating_sub(1));
        // Blocks before the cut keep their place, and so do those after it: the cut stays as a
        // tracked deletion until accepted.
        s.sel = crate::Selection { anchor: Pos::body(a.block, 0), focus: Pos::body(to, 0) };
        cmd(s, name, "edit.cut", json!({}))?;
        if s.pending_move.is_none() {
            return Err("the section couldn't be cut as a move".into());
        }
        let at = Pos::body(b.block, 0);
        let props = s.doc.body.get(b.block).and_then(|x| x.as_para()).map(|p| p.props.clone());
        s.sel = crate::Selection::caret(at.clone());
        s.join_next_undo();
        cmd(s, name, "edit.paste", json!({}))?;
        // The paste ends in the heading it went before, which keeps its own paragraph look (the
        // cut's last, empty piece would give it the look of the paragraph after the cut).
        let after = s.sel.focus.path.clone();
        if let (Some(props), Ok(p)) = (props, s.doc.para_mut(StoryRef::Body, &after)) {
            p.props = props;
        }
        s.sel = crate::Selection::caret(at);
        Ok(())
    });
    s.view.comments_pane = pane;
    r?;
    Ok(Applied {
        kind: EditKind::Insert,
        deleted: a.heading.clone(),
        inserted: a.heading.clone(),
        quote: b.heading,
        note: "moved".into(),
        comment: None,
        parts: Vec::new(),
    })
}

/// Where a new heading goes: after the section headed `after`, else after the one top-level
/// paragraph holding those words; empty = after the last section (or at the end).
fn insert_point(s: &Session, list: &[Section], after: &str) -> Result<usize, String> {
    if after.trim().is_empty() {
        return Ok(list.last().map_or(s.doc.body.len(), |x| x.range_end));
    }
    if let Some(x) = find(list, after) {
        return Ok(x.range_end);
    }
    let k = edits_key(after);
    let hits: Vec<usize> =
        s.doc.body.iter().enumerate().filter(|(_, b)| b.as_para().is_some_and(|p| edits_key(&p.final_text()).contains(&k))).map(|(i, _)| i).collect();
    match hits.as_slice() {
        [i] => Ok(i + 1),
        [] => Err("the place it goes isn't in the document".into()),
        _ => Err("the place it goes is in the document more than once".into()),
    }
}

/// Insert `text` as a heading of `level` (1-based) where [`insert_point`] says, as one tracked
/// insert by `name` and one undo step. Heading text only: nothing under it.
pub fn insert_heading(s: &mut Session, name: &str, after: &str, text: &str, level: u8) -> Result<Applied, String> {
    if let Some(why) = not_a_heading(text) {
        return Err(why.into());
    }
    let list = outline(s);
    if find(&list, text).is_some() {
        return Err("the document has that heading already".into());
    }
    let at = insert_point(s, &list, after)?;
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let style = format!("Heading {}", level.clamp(1, 6));
    let pane = s.view.comments_pane;
    let r = keeping_all(s, &text, |s| {
        // At the end of the paragraph before, so the new line starts a paragraph of its own;
        // before the first block, the heading's paragraph is split off the front of it.
        let prev = at.checked_sub(1).filter(|p| s.doc.body.get(*p).is_some_and(|b| b.as_para().is_some()));
        let head = match prev {
            Some(p) => {
                s.sel = crate::Selection::caret(Pos::body(p, para_len(s, p)));
                cmd(s, Some(name), "text.newParagraph", json!({}))?;
                s.join_next_undo();
                cmd(s, Some(name), "text.insert", json!({"text": text, "raw": true}))?;
                Pos::body(p + 1, 0)
            }
            None => {
                s.sel = crate::Selection::caret(Pos::body(at, 0));
                cmd(s, Some(name), "text.insert", json!({"text": text, "raw": true}))?;
                s.join_next_undo();
                cmd(s, Some(name), "text.newParagraph", json!({}))?;
                Pos::body(at, 0)
            }
        };
        s.sel = crate::Selection::caret(head);
        s.join_next_undo();
        cmd(s, Some(name), "para.style", json!({"style": style}))?;
        Ok(())
    });
    s.view.comments_pane = pane;
    r?;
    Ok(Applied {
        kind: EditKind::Insert,
        deleted: String::new(),
        inserted: text.clone(),
        quote: after.to_string(),
        note: String::new(),
        comment: None,
        parts: vec![(String::new(), text)],
    })
}
