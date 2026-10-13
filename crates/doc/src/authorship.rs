//! Authorship of accepted assistant text (Margin's A5; design in `docs/authorship-storage.md`).
//!
//! When the person accepts a suggestion an assistant made, the accepted text is wrapped in a
//! hidden bookmark named `_MarginAI_<8 hex>` ([`BOOKMARK_PREFIX`]) and a [`Span`] records who
//! wrote it (author, model), when it was accepted and the text as accepted. The bookmarks are
//! ordinary [`InlineObject::BookmarkStart`] / [`InlineObject::BookmarkEnd`] markers, so Word and
//! every WordCraft keep them and shrink or grow them with the text; the spans travel in a custom
//! XML part of the `.docx` ([`NAMESPACE`], written by the docx crate).
//!
//! The paragraphs are the source of truth: a span whose bookmark is gone is dropped on load
//! ([`reconcile`]), stray or duplicate markers are removed, and what the AI still wrote inside a
//! bookmark is decided by comparing the bookmark's text with the stored text ([`attribution`]):
//! characters a person changed inside the span count as the person's.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::para::{InlineObject, OBJ};
use crate::{Document, Paragraph, PartKind, Path, Pos, StoryRef};

/// Bookmark names that mark assistant text. The leading `_` hides them from Word's Bookmark
/// dialog (ECMA-376 §17.13.6 leaves the name free; Microsoft documents the convention).
pub const BOOKMARK_PREFIX: &str = "_MarginAI_";
/// The custom XML part's namespace.
pub const NAMESPACE: &str = "urn:margin:authorship";
/// Longest accepted text kept in a span (characters); longer spans keep only their hash.
pub const MAX_STORED_TEXT_CHARS: usize = 10_000;
/// Most spans read from one file.
pub const MAX_SPANS: usize = 10_000;
pub const MAX_NAME_CHARS: usize = 64;
pub const MAX_AUTHOR_CHARS: usize = 256;
pub const MAX_MODEL_CHARS: usize = 128;
pub const MAX_DATE_CHARS: usize = 64;
/// Longest edited stretch (either side) compared character by character; a longer one counts as
/// the person's as a whole.
pub const MAX_DIFF_CHARS: usize = 400;

/// Whether a bookmark name marks assistant text. Lenient on read: any `_MarginAI…` name counts
/// (the spike's files used `_MarginAI1`); new marks are always `_MarginAI_<8 hex>`.
pub fn is_mark(name: &str) -> bool {
    name.starts_with(BOOKMARK_PREFIX.trim_end_matches('_'))
}

/// A mark name from a seed: `_MarginAI_` and eight hex digits.
pub fn mark_name(seed: u64) -> String {
    format!("{BOOKMARK_PREFIX}{:08x}", (seed ^ (seed >> 32)) as u32)
}

/// Text as a span stores it: without object placeholders, and with characters XML cannot carry
/// (control characters other than tab and line feed, U+FFFE, U+FFFF) replaced by U+FFFD. The
/// live text is put through the same function before it is compared.
pub fn storable(text: &str) -> String {
    text.chars()
        .filter(|c| *c != OBJ)
        .map(|c| if c == '\t' || c == '\n' || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}') { c } else { '\u{FFFD}' })
        .collect()
}

/// FNV-1a (64-bit) of the text's UTF-8 bytes, as 16 hex digits.
pub fn hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn cap(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// An author name as written on the change: not normalised, only bounded.
fn cap_author(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(MAX_AUTHOR_CHARS).collect()
}

/// One accepted assistant span: the bookmark it lives in and what was accepted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Span {
    /// The bookmark's name (`_MarginAI_…`).
    pub name: String,
    /// The assistant's author name exactly as written on its tracked changes (never normalised:
    /// the name may be a model's display name).
    pub author: String,
    /// The model id when known (`claude-opus-5`, `mistral-large-latest`…), else empty.
    pub model: String,
    /// When the person accepted it (ISO-8601, UTC).
    pub accepted_at: String,
    /// The accepted text ([`storable`]) when it has at most [`MAX_STORED_TEXT_CHARS`]
    /// characters; `None` keeps only the hash (the span then counts as the AI's as a whole).
    pub text: Option<String>,
    /// [`hash`] of the accepted text.
    pub hash: String,
    /// Characters in the accepted text.
    pub len: usize,
}

impl Span {
    /// A span for `accepted_text` as it was accepted.
    pub fn new(name: &str, author: &str, model: &str, accepted_at: &str, accepted_text: &str) -> Span {
        let mut s = Span {
            name: cap(name, MAX_NAME_CHARS),
            author: cap_author(author),
            model: cap(model, MAX_MODEL_CHARS),
            accepted_at: cap(accepted_at, MAX_DATE_CHARS),
            ..Span::default()
        };
        s.set_text(accepted_text);
        s
    }

    /// A span read from a file that kept only the hash of its text.
    pub fn hash_only(name: &str, author: &str, model: &str, accepted_at: &str, hash: &str, len: usize) -> Span {
        Span {
            name: cap(name, MAX_NAME_CHARS),
            author: cap_author(author),
            model: cap(model, MAX_MODEL_CHARS),
            accepted_at: cap(accepted_at, MAX_DATE_CHARS),
            text: None,
            hash: cap(hash, 16),
            len,
        }
    }

    /// Record `accepted_text` as the span's text.
    pub fn set_text(&mut self, accepted_text: &str) {
        let t = storable(accepted_text);
        self.len = t.chars().count();
        self.hash = hash(&t);
        self.text = (self.len <= MAX_STORED_TEXT_CHARS).then_some(t);
    }

    /// Does the live text (already [`storable`]) still read as accepted?
    pub fn matches(&self, live: &str) -> bool {
        match &self.text {
            Some(t) => t == live,
            None => hash(live) == self.hash,
        }
    }
}

/// The document's spans (`Document::authorship`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Authorship {
    pub spans: Vec<Span>,
}

impl Authorship {
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
    pub fn get(&self, name: &str) -> Option<&Span> {
        self.spans.iter().find(|s| s.name == name)
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Span> {
        self.spans.iter_mut().find(|s| s.name == name)
    }
    /// Remove the span named `name`; whether there was one.
    pub fn remove(&mut self, name: &str) -> bool {
        let n = self.spans.len();
        self.spans.retain(|s| s.name != name);
        self.spans.len() != n
    }
}

// ---------------------------------------------------------------------------------------------
// Where the marks are

/// A mark's bookmark: the positions of its start and end markers (both in `start.story`, in the
/// same container, start before end).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkRange {
    pub name: String,
    pub start: Pos,
    pub end: Pos,
}

/// Every mark in the document, and the markers that don't make a mark.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkScan {
    /// Well-formed marks, document order.
    pub ranges: Vec<MarkRange>,
    /// Markers to remove: starts without an end, ends without a start, ends before their start,
    /// pairs crossing into another story or table cell, second and later markers of one name.
    pub stray: Vec<Pos>,
    /// One line per oddity, for the log.
    pub notes: Vec<String>,
}

/// Every story of the document, body first.
pub fn stories(doc: &Document) -> Vec<StoryRef> {
    std::iter::once(StoryRef::Body).chain(doc.parts.keys().map(|k| StoryRef::Part(*k))).collect()
}

/// Find every `_MarginAI_` bookmark and pair its markers.
pub fn scan(doc: &Document) -> MarkScan {
    let mut starts: BTreeMap<String, Vec<Pos>> = BTreeMap::new();
    let mut ends: BTreeMap<String, Vec<Pos>> = BTreeMap::new();
    for story in stories(doc) {
        for path in doc.para_paths(story) {
            let Some(p) = doc.para(story, &path) else { continue };
            for off in p.object_offsets() {
                match p.object_at(off) {
                    Some(InlineObject::BookmarkStart { name }) if is_mark(name) => {
                        starts.entry(name.clone()).or_default().push(Pos { story, path: path.clone(), off });
                    }
                    Some(InlineObject::BookmarkEnd { name }) if is_mark(name) => {
                        ends.entry(name.clone()).or_default().push(Pos { story, path: path.clone(), off });
                    }
                    _ => {}
                }
            }
        }
    }
    let names: BTreeSet<String> = starts.keys().chain(ends.keys()).cloned().collect();
    let mut out = MarkScan::default();
    for name in names {
        let s = starts.remove(&name).unwrap_or_default();
        let e = ends.remove(&name).unwrap_or_default();
        let Some(a) = s.first() else {
            out.stray.extend(e);
            out.notes.push(format!("{name}: end without start, removed"));
            continue;
        };
        // The first end after the first start, in the same story and container (not into or out
        // of a table cell).
        let b = e.iter().find(|b| **b > *a);
        match b {
            Some(b) if b.story == a.story && b.path.parent() == a.path.parent() => {
                out.ranges.push(MarkRange { name: name.clone(), start: a.clone(), end: b.clone() });
                let extra = s.len() - 1 + e.len() - 1;
                out.stray.extend(s.iter().skip(1).cloned());
                out.stray.extend(e.iter().filter(|x| *x != b).cloned());
                if extra > 0 {
                    out.notes.push(format!("{name}: {extra} extra marker(s) removed, the first pair kept"));
                }
            }
            Some(_) => {
                out.stray.extend(s);
                out.stray.extend(e);
                out.notes.push(format!("{name}: crosses into another story or table cell, removed"));
            }
            None => {
                out.stray.extend(s);
                out.stray.extend(e);
                out.notes.push(format!("{name}: start without end, removed"));
            }
        }
    }
    out.ranges.sort_by(|x, y| x.start.cmp(&y.start));
    out
}

/// Remove the bookmark markers at `positions` (only `_MarginAI_` markers are touched).
pub fn remove_markers(doc: &mut Document, mut positions: Vec<Pos>) {
    positions.sort();
    positions.dedup();
    for pos in positions.into_iter().rev() {
        let Ok(p) = doc.para_mut(pos.story, &pos.path) else { continue };
        let ours = matches!(p.object_at(pos.off), Some(InlineObject::BookmarkStart { name } | InlineObject::BookmarkEnd { name }) if is_mark(name));
        if ours {
            let _ = p.delete(pos.off, pos.off + OBJ.len_utf8());
        }
    }
}

/// A visible character of a span: its paragraph, byte offset (`None` for the break between two
/// paragraphs) and the character.
struct LiveChar {
    path: Path,
    off: Option<usize>,
    c: char,
}

/// The visible characters inside `r`: no object placeholders, no tracked deletions; a line feed
/// between paragraphs.
fn live_chars(doc: &Document, r: &MarkRange) -> Vec<LiveChar> {
    let mut out = Vec::new();
    let story = r.start.story;
    let mut first = true;
    for path in doc.para_paths(story).into_iter().filter(|p| *p >= r.start.path && *p <= r.end.path) {
        let Some(p) = doc.para(story, &path) else { continue };
        if !first {
            out.push(LiveChar { path: path.clone(), off: None, c: '\n' });
        }
        first = false;
        let a = if path == r.start.path { r.start.off.saturating_add(OBJ.len_utf8()) } else { 0 };
        let b = if path == r.end.path { r.end.off } else { p.len() };
        let deleted = p.deleted_ranges();
        for (i, c) in p.text.char_indices() {
            if i < a || i >= b || c == OBJ || deleted.iter().any(|d| d.contains(&i)) {
                continue;
            }
            out.push(LiveChar { path: path.clone(), off: Some(i), c });
        }
    }
    out
}

/// The text inside `r` as it reads, in [`storable`] form.
pub fn span_text(doc: &Document, r: &MarkRange) -> String {
    storable(&live_chars(doc, r).iter().map(|l| l.c).collect::<String>())
}

/// The visible text of `para[a..b]` ([`storable`]): no placeholders, no tracked deletions.
pub fn visible_text(para: &Paragraph, a: usize, b: usize) -> String {
    let deleted = para.deleted_ranges();
    storable(
        &para.text.char_indices().filter(|(i, _)| *i >= a && *i < b && !deleted.iter().any(|d| d.contains(i))).map(|(_, c)| c).collect::<String>(),
    )
}

// ---------------------------------------------------------------------------------------------
// Load repair

/// Make the spans and the bookmarks agree (`docs/authorship-storage.md` §6): a span whose
/// bookmark is missing or empty is dropped; stray, duplicate and crossing markers go; a
/// `_MarginAI_` bookmark no span describes is removed. Returns one line per repair.
pub fn reconcile(doc: &mut Document) -> Vec<String> {
    let scan = scan(doc);
    let mut log = scan.notes.clone();
    let mut remove = scan.stray.clone();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut kept = Vec::new();
    for span in std::mem::take(&mut doc.authorship.spans) {
        if !seen.insert(span.name.clone()) {
            log.push(format!("{}: listed twice, the second entry dropped", span.name));
            continue;
        }
        match scan.ranges.iter().find(|r| r.name == span.name) {
            None => log.push(format!("{} by {}: bookmark missing, entry dropped", span.name, span.author)),
            Some(r) if span_text(doc, r).is_empty() => {
                remove.push(r.start.clone());
                remove.push(r.end.clone());
                log.push(format!("{} by {}: bookmark empty, dropped", span.name, span.author));
            }
            Some(_) => kept.push(span),
        }
    }
    doc.authorship.spans = kept;
    for r in scan.ranges.iter().filter(|r| !seen.contains(&r.name)) {
        if doc.authorship.get(&r.name).is_none() {
            remove.push(r.start.clone());
            remove.push(r.end.clone());
            log.push(format!("{}: no entry for this bookmark, removed", r.name));
        }
    }
    if !remove.is_empty() {
        remove_markers(doc, remove);
    }
    log
}

// ---------------------------------------------------------------------------------------------
// Who wrote what

/// Per character of `live`: does it come from `stored` (the AI's), or did a person add it? The
/// common beginning and end match outright; the edited middle is compared character by
/// character when both sides are at most [`MAX_DIFF_CHARS`] long, else it is the person's.
pub fn diff_attribution(stored: &str, live: &str) -> Vec<bool> {
    let s: Vec<char> = stored.chars().collect();
    let l: Vec<char> = live.chars().collect();
    let mut out = vec![false; l.len()];
    let mut p = 0;
    while p < s.len() && p < l.len() && s.get(p) == l.get(p) {
        p += 1;
    }
    let mut q = 0;
    while q < s.len() - p && q < l.len() - p && s.get(s.len() - 1 - q) == l.get(l.len() - 1 - q) {
        q += 1;
    }
    for flag in out.iter_mut().take(p) {
        *flag = true;
    }
    for flag in out.iter_mut().skip(l.len() - q) {
        *flag = true;
    }
    let sm = s.get(p..s.len() - q).unwrap_or_default();
    let lm = l.get(p..l.len() - q).unwrap_or_default();
    if sm.is_empty() || lm.is_empty() || sm.len() > MAX_DIFF_CHARS || lm.len() > MAX_DIFF_CHARS {
        return out;
    }
    // Longest common subsequence of the two middles; its characters are the AI's.
    let (n, m) = (sm.len(), lm.len());
    let mut dp = vec![0u16; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let v = if sm.get(i) == lm.get(j) {
                dp.get(at(i + 1, j + 1)).copied().unwrap_or(0) + 1
            } else {
                dp.get(at(i + 1, j)).copied().unwrap_or(0).max(dp.get(at(i, j + 1)).copied().unwrap_or(0))
            };
            if let Some(cell) = dp.get_mut(at(i, j)) {
                *cell = v;
            }
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if sm.get(i) == lm.get(j) {
            if let Some(flag) = out.get_mut(p + j) {
                *flag = true;
            }
            i += 1;
            j += 1;
        } else if dp.get(at(i + 1, j)).copied().unwrap_or(0) >= dp.get(at(i, j + 1)).copied().unwrap_or(0) {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// One paragraph's split.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParaShare {
    /// Byte ranges of accepted AI text (merged, document order).
    pub ai: Vec<(usize, usize)>,
    /// Byte ranges of pending assistant insertions (tracked, not yet accepted).
    pub pending: Vec<(usize, usize)>,
    /// Visible characters (no placeholders, no tracked deletions).
    pub total_chars: usize,
    pub ai_chars: usize,
    pub pending_chars: usize,
    /// Accepted AI characters per author.
    pub authors: BTreeMap<String, usize>,
}

impl ParaShare {
    /// The assistant that wrote most of this paragraph's AI text.
    pub fn main_author(&self) -> Option<&str> {
        self.authors.iter().max_by_key(|(_, n)| **n).map(|(a, _)| a.as_str())
    }
}

/// One span as it stands now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanState {
    pub name: String,
    pub author: String,
    pub start: Pos,
    pub end: Pos,
    /// Characters inside the bookmark now.
    pub live_chars: usize,
    /// Of those, the AI's.
    pub ai_chars: usize,
    /// The text no longer reads as accepted.
    pub edited: bool,
}

/// Who wrote what, over the body, headers, footers, notes and text boxes (comments are not text
/// of the document). Characters in tracked deletions are not counted; a person's tracked
/// insertion inside a span is the person's; `is_assistant` says whose pending insertions count
/// as pending AI text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attribution {
    pub paras: BTreeMap<StoryRef, BTreeMap<Path, ParaShare>>,
    pub spans: Vec<SpanState>,
    pub total_chars: usize,
    pub ai_chars: usize,
    pub pending_chars: usize,
    pub authors: BTreeMap<String, usize>,
}

impl Attribution {
    pub fn para(&self, story: StoryRef, path: &Path) -> Option<&ParaShare> {
        self.paras.get(&story).and_then(|m| m.get(path))
    }
    /// Characters neither accepted AI text nor pending suggestions.
    pub fn people_chars(&self) -> usize {
        self.total_chars.saturating_sub(self.ai_chars).saturating_sub(self.pending_chars)
    }
}

const F_DEL: u8 = 1;
const F_PENDING: u8 = 2;
const F_AI: u8 = 4;
const F_OBJ: u8 = 8;

struct Flags {
    bits: Vec<u8>,
    author: Vec<u16>,
}

/// Merge byte positions with `bit` set (and neither deleted nor an object) into ranges.
fn ranges_of(p: &Paragraph, f: &Flags, want: impl Fn(u8) -> bool) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (i, c) in p.text.char_indices() {
        let b = f.bits.get(i).copied().unwrap_or(0);
        if b & (F_DEL | F_OBJ) != 0 || !want(b) {
            continue;
        }
        let end = i + c.len_utf8();
        match out.last_mut() {
            Some((_, e)) if *e == i => *e = end,
            _ => out.push((i, end)),
        }
    }
    out
}

/// Compute the split (see [`Attribution`]).
pub fn attribution(doc: &Document, is_assistant: &dyn Fn(&str) -> bool) -> Attribution {
    let mut flags: BTreeMap<(StoryRef, Path), Flags> = BTreeMap::new();
    let mut authors: Vec<String> = Vec::new();
    for story in stories(doc) {
        if let StoryRef::Part(id) = story
            && doc.parts.get(&id).is_some_and(|p| p.kind == PartKind::Comment)
        {
            continue;
        }
        for path in doc.para_paths(story) {
            let Some(p) = doc.para(story, &path) else { continue };
            let mut f = Flags { bits: vec![0; p.len()], author: vec![0; p.len()] };
            for (r, c) in p.run_ranges() {
                let mut b = 0u8;
                if c.del.is_some() {
                    b |= F_DEL;
                } else if let Some(rev) = c.ins
                    && doc.revisions.get(rev as usize).is_some_and(|v| is_assistant(&v.author))
                {
                    b |= F_PENDING;
                }
                if b != 0 {
                    for x in f.bits.iter_mut().take(r.end).skip(r.start) {
                        *x |= b;
                    }
                }
            }
            for (i, c) in p.text.char_indices() {
                if c == OBJ {
                    for k in 0..c.len_utf8() {
                        if let Some(x) = f.bits.get_mut(i + k) {
                            *x |= F_OBJ;
                        }
                    }
                }
            }
            flags.insert((story, path), f);
        }
    }
    let mut spans = Vec::new();
    for r in scan(doc).ranges {
        let Some(span) = doc.authorship.get(&r.name) else { continue };
        if let StoryRef::Part(id) = r.start.story
            && doc.parts.get(&id).is_some_and(|p| p.kind == PartKind::Comment)
        {
            continue;
        }
        let live = live_chars(doc, &r);
        let live_text: String = storable(&live.iter().map(|l| l.c).collect::<String>());
        let ai: Vec<bool> = match &span.text {
            Some(stored) if *stored == live_text => vec![true; live.len()],
            Some(stored) => diff_attribution(stored, &live_text),
            None => vec![true; live.len()],
        };
        let edited = !span.matches(&live_text);
        let idx = match authors.iter().position(|a| *a == span.author) {
            Some(i) => i,
            None => {
                authors.push(span.author.clone());
                authors.len() - 1
            }
        };
        let mut ai_chars = 0usize;
        for (k, lc) in live.iter().enumerate() {
            if !ai.get(k).copied().unwrap_or(false) {
                continue;
            }
            let Some(off) = lc.off else { continue };
            let Some(f) = flags.get_mut(&(r.start.story, lc.path.clone())) else { continue };
            let already = f.bits.get(off).is_some_and(|b| b & F_AI != 0);
            for k in 0..lc.c.len_utf8() {
                if let Some(x) = f.bits.get_mut(off + k) {
                    *x |= F_AI;
                }
                if !already && let Some(a) = f.author.get_mut(off + k) {
                    *a = (idx + 1).min(u16::MAX as usize) as u16;
                }
            }
            if !already {
                ai_chars += 1;
            }
        }
        spans.push(SpanState {
            name: r.name.clone(),
            author: span.author.clone(),
            start: r.start.clone(),
            end: r.end.clone(),
            live_chars: live.iter().filter(|l| l.off.is_some()).count(),
            ai_chars,
            edited,
        });
    }
    let mut out = Attribution { spans, ..Attribution::default() };
    for ((story, path), f) in &flags {
        let Some(p) = doc.para(*story, path) else { continue };
        let mut share = ParaShare {
            ai: ranges_of(p, f, |b| b & F_AI != 0),
            pending: ranges_of(p, f, |b| b & F_PENDING != 0 && b & F_AI == 0),
            ..ParaShare::default()
        };
        for (i, _) in p.text.char_indices() {
            let b = f.bits.get(i).copied().unwrap_or(0);
            if b & (F_DEL | F_OBJ) != 0 {
                continue;
            }
            share.total_chars += 1;
            if b & F_AI != 0 {
                share.ai_chars += 1;
                if let Some(name) = f.author.get(i).and_then(|a| authors.get((*a as usize).wrapping_sub(1))) {
                    *share.authors.entry(name.clone()).or_default() += 1;
                }
            } else if b & F_PENDING != 0 {
                share.pending_chars += 1;
            }
        }
        out.total_chars += share.total_chars;
        out.ai_chars += share.ai_chars;
        out.pending_chars += share.pending_chars;
        for (a, n) in &share.authors {
            *out.authors.entry(a.clone()).or_default() += n;
        }
        out.paras.entry(*story).or_default().insert(path.clone(), share);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::CharProps;
    use crate::{Revision, RevisionKind, para_block};

    fn doc_with(paras: &[&str]) -> Document {
        let mut d = Document::new();
        d.body = paras.iter().map(|t| para_block(Paragraph::with_text(t, CharProps::default()))).collect();
        d
    }

    /// Wrap `para.text[a..b]` in a mark named `name` and record its span.
    fn mark(d: &mut Document, para: usize, a: usize, b: usize, name: &str, author: &str) {
        let path = Path::top(para);
        let p = d.para_mut(StoryRef::Body, &path).unwrap();
        let text = p.text.get(a..b).unwrap().to_string();
        p.insert_object(b, InlineObject::BookmarkEnd { name: name.into() }, &CharProps::default()).unwrap();
        p.insert_object(a, InlineObject::BookmarkStart { name: name.into() }, &CharProps::default()).unwrap();
        d.authorship.spans.push(Span::new(name, author, "m", "2026-10-13T09:00:00Z", &text));
    }

    #[test]
    fn storable_and_hash_are_stable() {
        assert_eq!(storable("a\u{FFFC}b\tc\nd\u{B}e\r"), "ab\tc\nd\u{FFFD}e\u{FFFD}");
        assert_eq!(hash(""), "cbf29ce484222325");
        assert_eq!(hash("a"), "af63dc4c8601ec8c");
        assert!(is_mark("_MarginAI_0000abcd") && is_mark("_MarginAI1") && !is_mark("_Ref123") && !is_mark("marginai") && !is_mark("MarginAI"));
        assert_eq!(mark_name(0x1234_5678).len(), BOOKMARK_PREFIX.len() + 8);
    }

    #[test]
    fn a_long_span_keeps_only_its_hash() {
        let long = "x".repeat(MAX_STORED_TEXT_CHARS + 1);
        let s = Span::new("_MarginAI_1", "Claude", "m", "t", &long);
        assert!(s.text.is_none() && s.len == MAX_STORED_TEXT_CHARS + 1 && s.matches(&long) && !s.matches("y"));
        let short = Span::new("_MarginAI_2", "Claude", "m", "t", "short");
        assert_eq!(short.text.as_deref(), Some("short"));
    }

    #[test]
    fn diff_attributes_common_prefix_suffix_and_middle() {
        let f = |s: &str, l: &str| diff_attribution(s, l).iter().map(|b| if *b { 'A' } else { '.' }).collect::<String>();
        assert_eq!(f("abc", "abc"), "AAA");
        assert_eq!(f("Mist lifts off the river", "Mist lifts off the wide river"), "AAAAAAAAAAAAAAAAAAA.....AAAAA");
        assert_eq!(f("Seewis sits", "sits"), "AAAA");
        assert_eq!(f("the cat sat", "the dog sat"), "AAAA...AAAA");
        assert_eq!(f("abc", "xyz"), "...");
        assert_eq!(f("", "new"), "...");
        assert_eq!(f("gone", ""), "");
        // Two edits inside: the untouched characters between them stay the AI's.
        assert_eq!(f("one two three", "one 2 three!"), "AAAA.AAAAAA.");
        // A middle longer than the cap counts as the person's as a whole.
        let long_s = format!("a{}z", "b".repeat(MAX_DIFF_CHARS + 1));
        let long_l = format!("a{}z", "c".repeat(MAX_DIFF_CHARS + 1));
        let r = diff_attribution(&long_s, &long_l);
        assert!(r[0] && r[r.len() - 1] && !r[1..r.len() - 1].iter().any(|b| *b));
    }

    #[test]
    fn scan_pairs_markers_and_finds_strays() {
        let mut d = doc_with(&["Hello wide world", "Second paragraph"]);
        mark(&mut d, 0, 6, 10, "_MarginAI_00000001", "Claude");
        let sc = scan(&d);
        assert_eq!(sc.ranges.len(), 1);
        assert_eq!(span_text(&d, &sc.ranges[0]), "wide");
        assert!(sc.stray.is_empty() && sc.notes.is_empty());
        // An end without a start, and a start without an end.
        let p = d.para_mut(StoryRef::Body, &Path::top(1)).unwrap();
        p.insert_object(0, InlineObject::BookmarkEnd { name: "_MarginAI_00000002".into() }, &CharProps::default()).unwrap();
        p.insert_object(p.len(), InlineObject::BookmarkStart { name: "_MarginAI_00000003".into() }, &CharProps::default()).unwrap();
        let sc = scan(&d);
        assert_eq!(sc.ranges.len(), 1);
        assert_eq!(sc.stray.len(), 2);
        assert_eq!(sc.notes.len(), 2, "{:?}", sc.notes);
    }

    #[test]
    fn reconcile_drops_missing_collapsed_stale_duplicate_and_crossing_spans() {
        let mut d = doc_with(&["Hello wide world", "Second paragraph", "Third"]);
        mark(&mut d, 0, 6, 10, "_MarginAI_00000001", "Claude");
        // Entry without a bookmark.
        d.authorship.spans.push(Span::new("_MarginAI_000000aa", "Claude", "m", "t", "gone"));
        // Collapsed bookmark.
        mark(&mut d, 1, 3, 3, "_MarginAI_00000002", "Claude");
        // Bookmark without an entry (stale).
        let p = d.para_mut(StoryRef::Body, &Path::top(2)).unwrap();
        p.insert_object(5, InlineObject::BookmarkEnd { name: "_MarginAI_000000ff".into() }, &CharProps::default()).unwrap();
        p.insert_object(0, InlineObject::BookmarkStart { name: "_MarginAI_000000ff".into() }, &CharProps::default()).unwrap();
        // Same entry twice.
        let dup = d.authorship.spans[0].clone();
        d.authorship.spans.push(dup);
        // A second bookmark with the first's name (pasted copy) later in the document.
        let p = d.para_mut(StoryRef::Body, &Path::top(2)).unwrap();
        let n = p.len();
        p.insert_object(n, InlineObject::BookmarkEnd { name: "_MarginAI_00000001".into() }, &CharProps::default()).unwrap();
        p.insert_object(n, InlineObject::BookmarkStart { name: "_MarginAI_00000001".into() }, &CharProps::default()).unwrap();
        let log = reconcile(&mut d);
        assert_eq!(d.authorship.spans.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["_MarginAI_00000001"]);
        let sc = scan(&d);
        assert_eq!(sc.ranges.len(), 1, "only the first pair of the kept span remains: {sc:?}");
        assert!(sc.stray.is_empty());
        assert_eq!(span_text(&d, &sc.ranges[0]), "wide");
        assert_eq!(d.plain_text(StoryRef::Body), "Hello wide world\nSecond paragraph\nThird");
        assert_eq!(log.len(), 5, "{log:?}");
        // A span crossing into a table cell is removed.
        let mut t = doc_with(&["Before"]);
        let mut table = crate::table::Table::new(1, 1, 400.0);
        table.rows[0].cells[0].blocks = vec![para_block(Paragraph::with_text("In cell", CharProps::default()))];
        t.body.push(std::sync::Arc::new(crate::Block::Table(table)));
        t.body.push(para_block(Paragraph::new()));
        t.para_mut(StoryRef::Body, &Path::top(0))
            .unwrap()
            .insert_object(0, InlineObject::BookmarkStart { name: "_MarginAI_00000009".into() }, &CharProps::default())
            .unwrap();
        let cell = Path(vec![1, 0, 0, 0]);
        t.para_mut(StoryRef::Body, &cell)
            .unwrap()
            .insert_object(2, InlineObject::BookmarkEnd { name: "_MarginAI_00000009".into() }, &CharProps::default())
            .unwrap();
        t.authorship.spans.push(Span::new("_MarginAI_00000009", "Claude", "m", "t", "BeforeIn"));
        let log = reconcile(&mut t);
        assert!(t.authorship.is_empty() && scan(&t).ranges.is_empty(), "{log:?}");
        assert!(log.iter().any(|l| l.contains("crosses")), "{log:?}");
    }

    #[test]
    fn attribution_counts_ai_people_pending_and_edits() {
        let mut d = doc_with(&["Hello wide world", "Mine alone"]);
        mark(&mut d, 0, 6, 10, "_MarginAI_00000001", "Claude");
        let a = attribution(&d, &|n| n == "Claude");
        assert_eq!((a.total_chars, a.ai_chars, a.pending_chars, a.people_chars()), (26, 4, 0, 22));
        let p0 = a.para(StoryRef::Body, &Path::top(0)).unwrap();
        // "Hello " + start marker (3 bytes) → "wide" at 9..13.
        assert_eq!(p0.ai, vec![(9, 13)]);
        assert_eq!(p0.authors.get("Claude"), Some(&4));
        assert_eq!(p0.main_author(), Some("Claude"));
        assert!(a.spans.len() == 1 && !a.spans[0].edited);
        // The person edits inside the span: only the untouched letters stay the AI's.
        let p = d.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
        p.insert_text(9 + 2, "XX", &CharProps::default()).unwrap();
        let a = attribution(&d, &|n| n == "Claude");
        assert_eq!((a.ai_chars, a.people_chars()), (4, 24));
        assert!(a.spans[0].edited);
        assert_eq!(a.para(StoryRef::Body, &Path::top(0)).unwrap().ai, vec![(9, 11), (13, 15)]);
        // A pending assistant insertion counts separately; a tracked deletion is not counted.
        d.revisions.push(Revision::new(RevisionKind::Insert, "Claude", "t"));
        d.revisions.push(Revision::new(RevisionKind::Delete, "Ana", "t"));
        let p = d.para_mut(StoryRef::Body, &Path::top(1)).unwrap();
        p.insert_text(4, " (new)", &CharProps { ins: Some(0), ..Default::default() }).unwrap();
        p.format(0, 4, &|c| c.del = Some(1)).unwrap();
        let a = attribution(&d, &|n| n == "Claude");
        let p1 = a.para(StoryRef::Body, &Path::top(1)).unwrap();
        assert_eq!((p1.total_chars, p1.pending_chars, p1.ai_chars), (12, 6, 0));
        assert_eq!(p1.pending, vec![(4, 10)]);
        assert_eq!(a.pending_chars, 6);
        // 18 + 12 visible characters, 4 the AI's, 6 pending.
        assert_eq!((a.total_chars, a.people_chars()), (30, 20));
    }

    #[test]
    fn a_span_across_two_paragraphs_counts_both() {
        let mut d = doc_with(&["abc", "def"]);
        d.para_mut(StoryRef::Body, &Path::top(0))
            .unwrap()
            .insert_object(1, InlineObject::BookmarkStart { name: "_MarginAI_00000001".into() }, &CharProps::default())
            .unwrap();
        d.para_mut(StoryRef::Body, &Path::top(1))
            .unwrap()
            .insert_object(2, InlineObject::BookmarkEnd { name: "_MarginAI_00000001".into() }, &CharProps::default())
            .unwrap();
        d.authorship.spans.push(Span::new("_MarginAI_00000001", "Claude", "m", "t", "bcde"));
        let a = attribution(&d, &|_| false);
        // The paragraph break counts as a person's character between (the stored text had none).
        assert_eq!(a.ai_chars, 4);
        assert_eq!(a.para(StoryRef::Body, &Path::top(0)).unwrap().ai, vec![(4, 6)]);
        assert_eq!(a.para(StoryRef::Body, &Path::top(1)).unwrap().ai, vec![(0, 2)]);
        assert!(reconcile(&mut d).is_empty(), "a span over two paragraphs is valid");
    }
}
