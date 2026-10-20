//! O9 (OPTIMO.md §10): what I'm stuck needs besides the model call (`cowriter/stuck.rs` reads
//! the answer). The person's trigger, not the co-writer, creates the empty paragraph the caret
//! stands in ([`prepare`]); questions are pinned one by one when clicked ([`pin`]); "From your
//! notes" is rule-based, computed here from this document's comments and the folders the person
//! allowed ([`from_notes`]); nothing in it is sent to the model.

use std::collections::HashSet;

use serde_json::json;
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{InlineObject, Path, Pos, StoryRef};

use crate::Session;
use crate::assistant_edits::{self as edits, ScopeKind, ScopeText};

/// Grey and italic over the empty paragraph; never document text.
pub const PLACEHOLDER: &str = "Start anywhere — the questions on the right are there if you want them.";
/// The kind line over a pinned question.
pub const KIND: &str = "Question";
/// Most questions shown, and most notes of each source in "From your notes".
pub const MAX_QUESTIONS: usize = 5;
pub const MAX_SNIPPETS: usize = 3;
/// A longest question or snippet, in characters.
pub const MAX_QUESTION_CHARS: usize = 300;

/// Whether the paragraph at `at` holds no text: nothing but spaces and comment marks.
pub fn is_blank(s: &Session, at: &Pos) -> bool {
    let Some(p) = s.doc.para_at(at) else { return false };
    p.object_offsets().iter().all(|o| matches!(p.object_at(*o), Some(InlineObject::CommentStart { .. } | InlineObject::CommentEnd { .. })))
        && p.text.chars().all(|c| c == OBJ || c.is_whitespace())
}

fn end_of(s: &Session, story: StoryRef, path: &Path) -> Pos {
    Pos { story, path: path.clone(), off: s.doc.para(story, path).map_or(0, |p| p.text.len()) }
}

/// The entry point (also for Structure's empty sections, O7): put the caret in an empty paragraph
/// and return where. `at` names a paragraph (an empty section's heading or its empty body line);
/// `None` is the caret's section. A blank paragraph is used as it is; otherwise a new one is
/// started at the end of `at` (or of the section), as the person's own Enter would.
pub fn prepare(s: &mut Session, at: Option<&Path>) -> Result<Pos, String> {
    let story = s.sel.focus.story;
    let path = match at {
        Some(p) => p.clone(),
        None => {
            let here = Pos { story, path: s.sel.focus.path.clone(), off: 0 };
            if is_blank(s, &here) {
                s.sel = crate::Selection::caret(here.clone());
                return Ok(here);
            }
            // The last paragraph of the section, which an empty one there is the answer.
            let last = edits::scope(s, ScopeKind::Section).ok().and_then(|sc| sc.pieces.last().map(|p| p.path.clone()));
            last.unwrap_or_else(|| s.sel.focus.path.clone())
        }
    };
    let start = Pos { story, path: path.clone(), off: 0 };
    if s.doc.para_at(&start).is_none() {
        return Err("that paragraph is gone".into());
    }
    if is_blank(s, &start) {
        s.sel = crate::Selection::caret(start.clone());
        return Ok(start);
    }
    s.sel = crate::Selection::caret(end_of(s, story, &path));
    s.run("text.newParagraph", &json!({})).map_err(|e| e.to_string())?;
    let now = s.sel.focus.clone();
    Ok(Pos { off: 0, ..now })
}

/// The text I'm stuck reads for the caret's section; an empty document has none (so it is sent
/// as empty rather than refused).
pub fn scope(s: &Session) -> ScopeText {
    edits::scope(s, ScopeKind::Section).unwrap_or_else(|_| ScopeText {
        kind: ScopeKind::Section,
        story: s.sel.focus.story,
        pieces: Vec::new(),
        text: String::new(),
        trimmed: false,
        total_chars: 0,
        title: None,
    })
}

/// Whether the stuck state still holds: the caret rests in the blank paragraph at `at`.
pub fn still_here(s: &Session, at: &Pos) -> bool {
    s.sel.is_collapsed() && s.sel.focus.story == at.story && s.sel.focus.path == at.path && is_blank(s, at)
}

/// Pin `text` as a comment by `name` at `at` (the caret in the empty paragraph). The selection and
/// panels are as they were.
pub fn pin(s: &mut Session, name: &str, at: &Pos, text: &str) -> Result<u32, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > edits::MAX_NOTE_CHARS {
        return Err("a note needs a text of reasonable length".into());
    }
    if !s.assistant_comments {
        return Err("comments by assistants are off in Settings › Co-writer".into());
    }
    let at = s.doc.clamp(at);
    let (sel, pane) = (s.sel.clone(), s.view.comments_pane);
    let r = edits::run_allowed(s, name, "select.range", &json!({"anchor": at, "focus": at}))
        .and_then(|_| edits::run_allowed(s, name, "review.newComment", &json!({"text": text})));
    s.sel = crate::Selection { anchor: s.doc.clamp(&sel.anchor), focus: s.doc.clamp(&sel.focus) };
    s.view.comments_pane = pane;
    let r = r?;
    r.get("id").and_then(serde_json::Value::as_u64).and_then(|n| u32::try_from(n).ok()).ok_or_else(|| "the note was not made".into())
}

/// A relevant earlier note, with where it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    pub text: String,
    /// "Comment by Lena · on “…”" or a file's name.
    pub origin: String,
}

fn clip(t: &str, n: usize) -> String {
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let c: String = t.chars().take(n).collect();
    if t.chars().count() > n { format!("{}…", c.trim_end()) } else { c }
}

/// Words of four letters or more, lower-case.
fn keywords(t: &str) -> HashSet<String> {
    t.split(|c: char| !c.is_alphabetic()).filter(|w| w.chars().count() >= 4).map(str::to_lowercase).take(2000).collect()
}

fn shares(words: &HashSet<String>, t: &str) -> bool {
    words.is_empty() || keywords(t).iter().any(|w| words.contains(w))
}

/// "From your notes": earlier comments in this document by people (not by `name`), and lines of
/// files in the allowed `folders` whose name matches, that share a word with the section's text.
/// An empty section matches every comment. At most [`MAX_SNIPPETS`] of each.
pub fn from_notes(s: &Session, scope: &ScopeText, name: &str, folders: &[String]) -> Vec<Snippet> {
    let words = keywords(&format!("{} {}", scope.title.as_deref().unwrap_or_default(), scope.text));
    let mut out = Vec::new();
    for (id, pos) in crate::cmd::review::comment_list(s) {
        let Some(c) = s.doc.comments.get(&id) else { continue };
        if c.parent.is_some() || crate::authorship::is_assistant_name(&c.author, &[name]) {
            continue;
        }
        let text = s.doc.plain_text(StoryRef::Part(c.part));
        let on = pos.as_ref().and_then(|p| s.doc.para_at(p)).map(|p| p.final_text()).unwrap_or_default();
        if text.trim().is_empty() || !(shares(&words, &text) || shares(&words, &on)) {
            continue;
        }
        let origin = match on.trim() {
            "" => format!("Comment by {}", c.author),
            o => format!("Comment by {} · on “{}”", c.author, clip(o, 40)),
        };
        out.push(Snippet { text: clip(&text, MAX_QUESTION_CHARS), origin });
        if out.len() >= MAX_SNIPPETS {
            break;
        }
    }
    if folders.is_empty() || words.is_empty() {
        return out;
    }
    let mut files = 0;
    for path in crate::assistant_source::list(folders) {
        let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !shares(&words, &stem) || keywords(&stem).is_empty() {
            continue;
        }
        let Some(src) = path.to_str().and_then(|p| crate::assistant_source::read(p, folders).ok()) else { continue };
        if let Some(line) = src.text.lines().find(|l| keywords(l).iter().any(|w| words.contains(w))) {
            out.push(Snippet { text: clip(line, MAX_QUESTION_CHARS), origin: src.name });
        }
        files += 1;
        if files >= MAX_SNIPPETS {
            break;
        }
    }
    out
}
