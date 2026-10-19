//! Authorship of accepted assistant text (Optimo's A5): what the engine does when a suggestion
//! by an assistant is accepted, and how typing treats the marks afterwards. The model and the
//! file format live in `wordcraft_doc::authorship` and the docx crate.
//!
//! * **Who is an assistant.** An author name starting with [`AI_PREFIX`] ("AI · "): every
//!   assistant writes under one ([`with_ai_prefix`]), so this holds on any computer and in other
//!   word processors. Older suggestions written without the prefix are recognised by the legacy
//!   list, [`Session::assistant_authors`]: names the host syncs (Settings › Co-writer's earlier
//!   names, persisted on this Mac) plus every name an assistant has written under in this
//!   session, compared by [`name_matches`]. Every decision goes through
//!   [`Session::is_assistant_author`] ([`is_assistant_name`]); nothing compares names elsewhere.
//! * **Accept.** Before a paragraph's changes are accepted, each assistant insertion in the
//!   accepted range is wrapped in a `_OptimoAI_<8 hex>` bookmark ([`wrap_before_accept`]); after,
//!   the text inside is recorded as a span with the author as written on the change, the model
//!   the host configured, and the time ([`record_accepted`]). A new span next to, or inside, an
//!   unedited span by the same author extends that span instead. Rejected changes leave nothing.
//! * **Typing.** Text typed at the end of a span goes after its bookmark end, as Word does for
//!   bookmarks ([`past_span_end`]): a person's continuation is the person's. Only `_OptimoAI_`
//!   bookmarks are treated this way; ordinary bookmarks keep upstream's behaviour.

use wordcraft_doc::authorship::{self as model, MarkRange, Span, is_mark, mark_name, visible_text};
use wordcraft_doc::para::{InlineObject, OBJ};
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, Path, Pos, StoryRef};

use crate::{CmdError, Session};

/// The assistant's author name while the host has set none (Settings › Co-writer S4).
pub const DEFAULT_ASSISTANT_NAME: &str = "Claude";

/// The prefix every assistant author name carries: "AI", a space, a middle dot, a space.
pub const AI_PREFIX: &str = "AI · ";

/// `name` with [`AI_PREFIX`] in front (never doubled); empty stays empty.
pub fn with_ai_prefix(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() || n.starts_with(AI_PREFIX) { n.to_string() } else { format!("{AI_PREFIX}{n}") }
}

/// The one rule for "is this author an assistant?": the name starts with [`AI_PREFIX`] (exactly:
/// "Aiko" or "AI: x" don't), or matches one of the `legacy` names ([`name_matches`]).
pub fn is_assistant_name<S: AsRef<str>>(author: &str, legacy: &[S]) -> bool {
    author.trim_start().starts_with(AI_PREFIX) || legacy.iter().any(|n| name_matches(n.as_ref(), author))
}

/// The one rule for comparing author names: trimmed, case-insensitive, never empty.
pub fn name_matches(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

/// Is `name` (trimmed) a `_OptimoAI_` mark name? Outside assistants may not make or remove one.
pub fn is_mark_name(name: &str) -> bool {
    is_mark(name.trim())
}

/// An assistant author as chips and the status bar show it: without an `AI · ` prefix.
pub fn display_name(author: &str) -> &str {
    let a = author.trim();
    a.strip_prefix(AI_PREFIX).unwrap_or(a).trim()
}

impl Session {
    /// Is `author` (the name on a change or comment) an assistant?
    pub fn is_assistant_author(&self, author: &str) -> bool {
        is_assistant_name(author, &self.assistant_authors)
    }

    /// Remember `name` as an assistant's author name.
    pub fn add_assistant_author(&mut self, name: &str) {
        if !name.trim().is_empty() && !self.is_assistant_author(name) {
            self.assistant_authors.push(name.trim().to_string());
        }
    }

    /// Who wrote what, as the view and the status bar show it.
    pub fn authorship(&self) -> model::Attribution {
        model::attribution(&self.doc, &|a| self.is_assistant_author(a))
    }
}

/// The position typing at `at` goes to: past the end markers of assistant spans sitting there.
pub fn past_span_end(doc: &Document, at: &Pos) -> Pos {
    let Some(p) = doc.para_at(at) else { return at.clone() };
    let mut off = at.off;
    while let Some(InlineObject::BookmarkEnd { name }) = p.object_at(off)
        && is_mark(name)
    {
        off += OBJ.len_utf8();
    }
    Pos { off, ..at.clone() }
}

/// A name no bookmark in `doc` has.
fn fresh_name(s: &Session, path: &Path, off: usize, taken: &[String]) -> String {
    let names: std::collections::BTreeSet<String> =
        doc_mark_names(&s.doc).into_iter().chain(s.doc.authorship.spans.iter().map(|x| x.name.clone())).chain(taken.iter().cloned()).collect();
    let mut seed = crate::cmd::now_unix().wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ s.rev().wrapping_mul(0x2545_F491_4F6C_DD1D)
        ^ (off as u64)
        ^ ((path.last() as u64) << 20);
    for _ in 0..64 {
        let n = mark_name(seed);
        if !names.contains(&n) {
            return n;
        }
        seed = seed.wrapping_add(0x9E37_79B9);
    }
    mark_name(seed)
}

/// `pos` as it reads once marks were added after `before` was taken: a position after a new
/// start marker, or at or after a new end marker, moves past it, so a caret at the end of
/// accepted text sits after the span (where typing goes) and never inside a marker.
pub fn map_past_new_marks(doc: &Document, before: &[String], pos: &Pos) -> Pos {
    let mut markers: Vec<(usize, bool)> = Vec::new();
    for r in model::scan(doc).ranges {
        if before.contains(&r.name) {
            continue;
        }
        if r.start.story == pos.story && r.start.path == pos.path {
            markers.push((r.start.off, true));
        }
        if r.end.story == pos.story && r.end.path == pos.path {
            markers.push((r.end.off, false));
        }
    }
    markers.sort_unstable();
    let mut off = pos.off;
    for (m, is_start) in markers {
        if m < off || (m == off && !is_start) {
            off = off.saturating_add(OBJ.len_utf8());
        }
    }
    Pos { off, ..pos.clone() }
}

/// The names of every mark in `doc` (start or end marker seen).
pub fn mark_names(doc: &Document) -> Vec<String> {
    doc_mark_names(doc)
}

fn doc_mark_names(doc: &Document) -> Vec<String> {
    let mut out = Vec::new();
    for story in model::stories(doc) {
        for path in doc.para_paths(story) {
            let Some(p) = doc.para(story, &path) else { continue };
            for off in p.object_offsets() {
                if let Some(InlineObject::BookmarkStart { name } | InlineObject::BookmarkEnd { name }) = p.object_at(off)
                    && is_mark(name)
                {
                    out.push(name.clone());
                }
            }
        }
    }
    out
}

/// Before `from..to` of the paragraph at `path` is accepted: wrap every assistant insertion in
/// that range in a new mark. Returns `to` moved past the markers inserted, and the new names in
/// document order (empty when nothing of an assistant's is being accepted).
pub fn wrap_before_accept(s: &mut Session, story: StoryRef, path: &Path, from: usize, to: usize) -> Result<(usize, Vec<String>), CmdError> {
    let Some(para) = s.doc.para(story, path) else { return Ok((to, Vec::new())) };
    // Assistant insertions (not deleted again) clipped to the range, adjacent ones merged.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (r, c) in para.run_ranges() {
        if r.end <= from || r.start >= to || c.del.is_some() {
            continue;
        }
        let by_assistant = c.ins.and_then(|i| s.doc.revisions.get(i as usize)).is_some_and(|v| s.is_assistant_author(&v.author));
        if !by_assistant {
            continue;
        }
        let (a, b) = (r.start.max(from), r.end.min(to));
        match ranges.last_mut() {
            Some((_, e)) if *e == a => *e = b,
            _ => ranges.push((a, b)),
        }
    }
    // Only ranges with visible text; markers of other kinds around them are left outside.
    ranges.retain(|(a, b)| !visible_text(para, *a, *b).is_empty());
    if ranges.is_empty() {
        return Ok((to, Vec::new()));
    }
    let mut names: Vec<String> = Vec::new();
    for (a, _) in &ranges {
        let n = fresh_name(s, path, *a, &names);
        names.push(n);
    }
    let mut end = to;
    for ((a, b), name) in ranges.iter().zip(names.iter()).rev() {
        let para = s.doc.para_mut(story, path)?;
        // The markers take the inserted text's formatting (its revision is cleared with it).
        let props: CharProps = para.props_of_char(*a).clone();
        para.insert_object(*b, InlineObject::BookmarkEnd { name: name.clone() }, &props)?;
        para.insert_object(*a, InlineObject::BookmarkStart { name: name.clone() }, &props)?;
        end = end.saturating_add(2 * OBJ.len_utf8());
    }
    Ok((end, names))
}

/// Start and end marker offsets of the mark `name` in `p`, when both are there.
fn markers_in(p: &wordcraft_doc::Paragraph, name: &str) -> Option<(usize, usize)> {
    let mut start = None;
    let mut end = None;
    for off in p.object_offsets() {
        match p.object_at(off) {
            Some(InlineObject::BookmarkStart { name: n }) if n == name && start.is_none() => start = Some(off),
            Some(InlineObject::BookmarkEnd { name: n }) if n == name && end.is_none() => end = Some(off),
            _ => {}
        }
    }
    match (start, end) {
        (Some(a), Some(b)) if a < b => Some((a, b)),
        _ => None,
    }
}

/// Spans wholly inside the paragraph at `path`, as (name, start marker, end marker).
fn spans_in(s: &Session, story: StoryRef, path: &Path) -> Vec<(String, usize, usize)> {
    let Some(p) = s.doc.para(story, path) else { return Vec::new() };
    let mut out = Vec::new();
    for off in p.object_offsets() {
        if let Some(InlineObject::BookmarkStart { name }) = p.object_at(off)
            && is_mark(name)
            && s.doc.authorship.get(name).is_some()
            && let Some((a, b)) = markers_in(p, name)
            && a == off
        {
            out.push((name.clone(), a, b));
        }
    }
    out
}

fn remove_marker(s: &mut Session, story: StoryRef, path: &Path, off: usize) -> Result<(), CmdError> {
    let p = s.doc.para_mut(story, path)?;
    if matches!(p.object_at(off), Some(InlineObject::BookmarkStart { name } | InlineObject::BookmarkEnd { name }) if is_mark(name)) {
        p.delete(off, off + OBJ.len_utf8())?;
    }
    Ok(())
}

/// After the accept: record the spans the new marks hold. `author` is the name on the accepted
/// change; a new span that touches or sits inside an unedited span of the same author in the
/// same paragraph extends that span instead of starting another.
pub fn record_accepted(s: &mut Session, story: StoryRef, path: &Path, names: &[String], author: &str, accepted_at: &str) -> Result<(), CmdError> {
    let model_id = s.assistant_model.clone();
    for name in names {
        let Some(p) = s.doc.para(story, path) else { continue };
        let Some((ns, ne)) = markers_in(p, name) else { continue };
        let text = visible_text(p, ns + OBJ.len_utf8(), ne);
        if text.is_empty() {
            remove_marker(s, story, path, ne)?;
            remove_marker(s, story, path, ns)?;
            continue;
        }
        // Extend a neighbouring or enclosing unedited span of the same author.
        let step = OBJ.len_utf8();
        let mut merged = false;
        for (ename, es, ee) in spans_in(s, story, path) {
            if ename == *name {
                continue;
            }
            let Some(existing) = s.doc.authorship.get(&ename) else { continue };
            if !name_matches(&existing.author, author) {
                continue;
            }
            let p = match s.doc.para(story, path) {
                Some(p) => p,
                None => break,
            };
            // The existing span's text without the new one must still read as accepted. Then the
            // two inner markers go and the new span's outer marker takes the existing name.
            let (without, drop, rename): (String, [usize; 2], Option<usize>) = if es < ns && ne < ee {
                (format!("{}{}", visible_text(p, es + step, ns), visible_text(p, ne + step, ee)), [ne, ns], None)
            } else if ee + step == ns {
                (visible_text(p, es + step, ee), [ns, ee], Some(ne))
            } else if ne + step == es {
                (visible_text(p, es + step, ee), [es, ne], Some(ns))
            } else {
                continue;
            };
            if !existing.matches(&without) {
                continue;
            }
            if let Some(off) = rename
                && let Ok(p) = s.doc.para_mut(story, path)
                && let Some(InlineObject::BookmarkStart { name: n } | InlineObject::BookmarkEnd { name: n }) = p.object_at_mut(off)
            {
                *n = ename.clone();
                p.touch();
            }
            let (hi, lo) = (drop[0].max(drop[1]), drop[0].min(drop[1]));
            remove_marker(s, story, path, hi)?;
            remove_marker(s, story, path, lo)?;
            let Some(p) = s.doc.para(story, path) else { break };
            let Some((a, b)) = markers_in(p, &ename) else { break };
            let whole = visible_text(p, a + step, b);
            if let Some(e) = s.doc.authorship.get_mut(&ename) {
                e.set_text(&whole);
                e.accepted_at = accepted_at.chars().take(model::MAX_DATE_CHARS).collect();
                if !model_id.is_empty() {
                    e.model = model_id.chars().take(model::MAX_MODEL_CHARS).collect();
                }
            }
            merged = true;
            break;
        }
        if !merged {
            s.doc.authorship.spans.push(Span::new(name, author, &model_id, accepted_at, &text));
        }
    }
    Ok(())
}

/// The author of the assistant insertion(s) in `from..to` (the first one found), for the span.
pub fn accepted_author(s: &Session, story: StoryRef, path: &Path, from: usize, to: usize) -> Option<String> {
    let p = s.doc.para(story, path)?;
    p.run_ranges()
        .filter(|(r, c)| r.end > from && r.start < to && c.del.is_none())
        .find_map(|(_, c)| c.ins.and_then(|i| s.doc.revisions.get(i as usize)).filter(|v| s.is_assistant_author(&v.author)).map(|v| v.author.clone()))
}

/// The marks of the document as a list (for `authorship.spans` and tests).
pub fn ranges(doc: &Document) -> Vec<MarkRange> {
    model::scan(doc).ranges
}
