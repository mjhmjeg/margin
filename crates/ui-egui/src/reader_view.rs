//! O6 (OPTIMO.md §7, R1–R4): Read as a reader and Check facts and names in the margin.
//!
//! * **Read as a reader** (⌥⌘R, ⌘Y): the top of the right margin asks who the reader is — "Read
//!   as" and a popup of presets, or Custom… for the person's own words — then Read. The last
//!   choice is kept per document for the session. After a reading the header keeps the popup
//!   (choose another reader to read again) and sums up: "4 notes · 2 questions · 1 strong line".
//! * **Note actions** by kind: Question, Unclear and Rhythm get Reply · Alternatives (opens O5 on
//!   that sentence) · Done; a Strong line gets Thanks; a Check note with a clear fix gets
//!   "Use 34" · "Keep thirty". "Use …" is the only thing here that changes the document: one
//!   tracked replacement by the AI author the note was made under, then the note is done.
//! * **Check** reads the person's files only from the folders allowed in Settings › Co-writer;
//!   the UI puts them in the run's options (never from a caller).

use std::collections::HashMap;

use egui::{Pos2, Rect, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::assistant_edits::{self as edits, Edit, EditKind, ScopeKind};
use wordcraft_engine::cowriter::{Output, Verb};
use wordcraft_engine::doc::Pos;

use crate::WordApp;
use crate::cowriter::Status;
use crate::mac_controls as mc;
use crate::theme::Tokens;

/// The reader popup's presets; the first is the default.
pub const READERS: [&str; 5] = ["A first-time reader", "A busy manager", "An expert", "A sceptic", "A funder"];
const CUSTOM: &str = "Custom…";
/// Options kept for notes (the oldest go first).
const MAX_KEPT: usize = 2_000;

/// Read as a reader's and Check's state in the margin.
#[derive(Default)]
pub struct State {
    /// The reader chooser is open at the top of the margin, for this scope.
    asking: Option<ScopeKind>,
    /// Open the popup's list the next time it is drawn (`{"open": true}`).
    open_list: bool,
    /// The last reader per document, and the scope it read.
    readers: HashMap<u64, (String, ScopeKind)>,
    /// The Custom… field, while it is open.
    custom: Option<String>,
    /// One-click options per (document, note text): (the quote, the options).
    options: HashMap<(u64, String), (String, Vec<String>)>,
    /// The note whose reply field is open, and whether it still wants the focus.
    replying: Option<(u32, bool)>,
}

/// Before `cowriter::start` runs `verb`: Read without a reader opens the chooser instead (the
/// answer to return); Check gets the allowed folders.
pub fn before_start(app: &mut WordApp, verb: Verb, kind: ScopeKind, options: &mut Value) -> Option<Result<Value, String>> {
    if !options.is_object() {
        *options = json!({});
    }
    let doc = app.session.document_id();
    match verb {
        Verb::ReadAsReader => {
            let who = options.get("reader").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string();
            let st = &mut app.cowriter.reader;
            if who.is_empty() {
                st.asking = Some(kind);
                st.open_list = options.get("open").and_then(Value::as_bool) == Some(true);
                app.cowriter.hide_summary = false;
                crate::mac_ui::set_margin(app, true);
                return Some(Ok(json!({"asking": true})));
            }
            st.asking = None;
            st.custom = None;
            st.readers.insert(doc, (who, kind));
        }
        Verb::CheckFacts => {
            let folders = app.ui.assistant.allowed_folders();
            if let Some(m) = options.as_object_mut() {
                m.insert("folders".into(), json!(folders));
            }
        }
        _ => {}
    }
    None
}

/// Keep the one-click options of a run's notes (the runner lands notes without them).
pub fn remember(app: &mut WordApp, doc: u64, outputs: &[Output]) {
    let kept = &mut app.cowriter.reader.options;
    for o in outputs {
        if let Output::Comment { anchor, text, options, .. } = o
            && !options.is_empty()
        {
            if kept.len() >= MAX_KEPT {
                kept.clear();
            }
            kept.insert((doc, text.trim().to_string()), (anchor.clone(), options.clone()));
        }
    }
}

/// The reader for `doc`: the last one chosen, else the first preset.
fn reader(app: &WordApp, doc: u64) -> (String, ScopeKind) {
    app.cowriter.reader.readers.get(&doc).cloned().unwrap_or_else(|| (READERS[0].to_string(), ScopeKind::Section))
}

fn last_run(app: &WordApp) -> Option<&crate::cowriter::Run> {
    app.cowriter.runs(app.session.document_id()).last()
}

/// The margin's top is Read as a reader's: the chooser is open, or the last run was a reading.
pub fn active(app: &WordApp) -> bool {
    app.cowriter.reader.asking.is_some() || last_run(app).is_some_and(|r| r.verb == Verb::ReadAsReader && !app.cowriter.hide_summary)
}

/// "4 notes · 2 questions · 1 strong line".
pub fn counts(kinds: &[&str]) -> String {
    let n = |k: &str| kinds.iter().filter(|x| **x == k).count();
    let plural = |n: usize, one: &str, many: &str| crate::i18n::fmt(tl!(if n == 1 { one } else { many }), &[("n", &n.to_string())]);
    let mut parts = vec![plural(kinds.len(), "{n} note", "{n} notes")];
    for (k, one, many) in [
        ("Question", "{n} question", "{n} questions"),
        ("Unclear", "{n} unclear", "{n} unclear"),
        ("Rhythm", "{n} rhythm", "{n} rhythm"),
        ("Strong line", "{n} strong line", "{n} strong lines"),
    ] {
        if n(k) > 0 {
            parts.push(plural(n(k), one, many));
        }
    }
    parts.join(" · ")
}

enum Act {
    Choose(String),
    Custom,
    Read,
    NotNow,
}

/// R1 at the top of the margin: "Read as" and the reader popup; the Read button while asking,
/// else the last reading's counts and report. Returns its bottom.
pub fn paint_margin(app: &mut WordApp, ui: &mut Ui, at: Pos2, w: f32) -> f32 {
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let pad = 12.0;
    let doc = app.session.document_id();
    let (current, kind) = reader(app, doc);
    let rect = Rect::from_min_size(pos2(at.x + pad, at.y), vec2(w - pad, 4000.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)).id_salt("reader_view"));
    let ui = &mut child;
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    let font = mc::font();
    let mut acts = Vec::new();
    ui.label(egui::RichText::new(tl!("Read as")).font(font.clone()).color(m.assistant));
    let mut items: Vec<String> = READERS.iter().map(|r| tl!(r).to_string()).collect();
    let shown = READERS.iter().find(|r| **r == current).map_or_else(|| current.clone(), |r| tl!(r).to_string());
    if !READERS.contains(&current.as_str()) {
        items.push(current.clone());
    }
    items.push(tl!(CUSTOM).to_string());
    if std::mem::take(&mut app.cowriter.reader.open_list) {
        mc::request_open(ui.ctx(), "reader_popup");
    }
    if let Some(c) = mc::popup_button(ui, "reader_popup", w - pad, &shown, &items, false) {
        if c == tl!(CUSTOM) {
            acts.push(Act::Custom);
        } else {
            let english = READERS.iter().find(|r| tl!(r) == c).map_or(c, |r| (*r).to_string());
            acts.push(Act::Choose(english));
        }
    }
    if let Some(text) = &mut app.cowriter.reader.custom {
        let r = ui.add(
            egui::TextEdit::singleline(text)
                .hint_text(tl!("Who is reading? e.g. a new volunteer"))
                .font(font.clone())
                .desired_width(w - pad)
                .char_limit(120),
        );
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            acts.push(Act::Read);
        }
    }
    let working = app.cowriter.working() && last_run(app).is_some_and(|r| r.verb == Verb::ReadAsReader && r.status == Status::Working);
    if app.cowriter.reader.asking.is_some() {
        ui.horizontal(|ui| {
            if mc::button(ui, "Read", true, !app.cowriter.working()).clicked() {
                acts.push(Act::Read);
            }
            if mc::link(ui, "Not now").clicked() {
                acts.push(Act::NotNow);
            }
        });
    } else if let Some(r) = last_run(app).filter(|r| r.verb == Verb::ReadAsReader) {
        if working {
            ui.label(egui::RichText::new(tl!("Reading…")).font(font.clone()).color(t.text_dim));
        } else {
            let kinds: Vec<&str> = r.notes.iter().map(|(_, k)| k.as_str()).collect();
            ui.label(egui::RichText::new(counts(&kinds)).font(font.clone()).color(t.text_dim));
        }
        if !r.report.trim().is_empty() {
            ui.add(egui::Label::new(egui::RichText::new(&r.report).font(font.clone()).color(t.text)).wrap());
        }
        for l in &r.lines {
            ui.add(egui::Label::new(egui::RichText::new(l).font(font.clone()).color(t.text_dim)).wrap());
        }
    }
    let bottom = ui.min_rect().max.y;
    crate::review_panel::mark_line(ui.painter(), pos2(at.x + 1.0, at.y + 2.0), pos2(at.x + 1.0, bottom - 2.0), m.assistant, true);
    for a in acts {
        let st = &mut app.cowriter.reader;
        match a {
            Act::Choose(who) => {
                st.custom = None;
                st.asking = Some(st.asking.unwrap_or(kind));
                st.readers.insert(doc, (who, st.asking.unwrap_or(kind)));
            }
            Act::Custom => {
                st.custom = Some(String::new());
                st.asking = Some(st.asking.unwrap_or(kind));
            }
            Act::NotNow => {
                st.asking = None;
                st.custom = None;
            }
            Act::Read => {
                let who = st.custom.clone().filter(|c| !c.trim().is_empty()).unwrap_or_else(|| reader(app, doc).0);
                let scope = app.cowriter.reader.asking.unwrap_or(kind);
                if let Err(e) = crate::cowriter::start(app, Verb::ReadAsReader, scope, json!({"reader": who})) {
                    app.status(e);
                }
            }
        }
    }
    bottom
}

/// The action links under a note of `kind` with `text`.
pub fn links(app: &WordApp, kind: &str, text: &str) -> Vec<String> {
    let doc = app.session.document_id();
    match kind {
        "Question" | "Unclear" | "Rhythm" => vec!["Reply".into(), "Alternatives".into(), "Done".into()],
        "Strong line" => vec!["Thanks".into()],
        "Check" => match app.cowriter.reader.options.get(&(doc, text.trim().to_string())) {
            Some((_, o)) => o.clone(),
            None => vec!["Done".into(), "Remove".into()],
        },
        _ => vec!["Done".into(), "Remove".into()],
    }
}

/// The quote as it reads after "Use …" on a note with `options` ("Use 34", "Keep thirty").
pub fn fixed(quote: &str, options: &[String]) -> Option<String> {
    let new = options.iter().find_map(|o| o.strip_prefix("Use "))?;
    let old = options.iter().find_map(|o| o.strip_prefix("Keep "))?;
    (quote.matches(old).count() == 1).then(|| quote.replacen(old, new, 1))
}

/// A note's action other than Done and Remove: `label` on note `id` anchored at `anchor`.
pub fn act(app: &mut WordApp, id: u32, anchor: &Pos, label: &str) {
    let resolve = |app: &mut WordApp| {
        let _ = app.run("review.resolveComment", json!({"id": id, "value": true}));
    };
    match label {
        "Thanks" => resolve(app),
        "Reply" => app.cowriter.reader.replying = Some((id, true)),
        "Alternatives" => {
            app.session.sel = wordcraft_engine::Selection::caret(app.session.doc.clamp(anchor));
            if let Err(e) = crate::cowriter::start(app, Verb::Alternatives, ScopeKind::Selection, json!({})) {
                app.status(e);
            }
        }
        l if l.starts_with("Keep ") => resolve(app),
        l if l.starts_with("Use ") => match choose(app, id) {
            Ok(()) => resolve(app),
            Err(e) => app.status(e),
        },
        _ => {}
    }
}

/// "Use …" on Check note `id`: the fix as one tracked replacement by the note's AI author.
pub fn choose(app: &mut WordApp, id: u32) -> Result<(), String> {
    let doc = app.session.document_id();
    let c = app.session.doc.comments.get(&id).ok_or_else(|| tl!("That note is gone.").to_string())?;
    let author = c.author.clone();
    let text = app.session.doc.plain_text(wordcraft_doc::StoryRef::Part(c.part)).trim().to_string();
    let (quote, options) = app.cowriter.reader.options.get(&(doc, text)).cloned().ok_or_else(|| tl!("That note has no fix to use.").to_string())?;
    let with = fixed(&quote, &options).ok_or_else(|| tl!("That note has no fix to use.").to_string())?;
    let scope = edits::scope(&app.session, ScopeKind::Document).map_err(|e| e.to_string())?;
    let done = edits::apply(&mut app.session, &author, &scope, &[Edit { kind: EditKind::Replace, quote, with, note: String::new() }]);
    if done.applied.is_empty() {
        return Err(done.log.into_iter().next().unwrap_or_else(|| tl!("Nothing changed.").into()));
    }
    Ok(())
}

/// The reply field under note `id` while it is open; returns its bottom.
pub fn paint_reply(app: &mut WordApp, ui: &mut Ui, at: Pos2, w: f32, id: u32) -> Option<f32> {
    let (on, focus) = app.cowriter.reader.replying?;
    if on != id {
        return None;
    }
    app.cowriter.reader.replying = Some((id, false));
    let pad = 12.0;
    let rect = Rect::from_min_size(pos2(at.x + pad, at.y), vec2(w - pad, 400.0));
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)).id_salt(("reader_reply", id)));
    if crate::panes::reply_editor(app, &mut child, id, Some(mc::font()), focus) {
        app.cowriter.reader.replying = None;
    }
    Some(child.min_rect().max.y)
}

#[cfg(test)]
mod tests {
    use wordcraft_engine::Session;

    use super::*;
    use crate::Services;

    #[test]
    fn counts_read_like_the_design() {
        assert_eq!(counts(&["Question", "Question", "Strong line", "Unclear"]), "4 notes · 2 questions · 1 unclear · 1 strong line");
        assert_eq!(counts(&["Strong line"]), "1 note · 1 strong line");
        assert_eq!(counts(&[]), "0 notes");
    }

    #[test]
    fn read_without_a_reader_asks_first_and_remembers_the_choice() {
        let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
        let mut o = json!({});
        assert!(before_start(&mut a, Verb::ReadAsReader, ScopeKind::Document, &mut o).is_some());
        assert!(active(&a) && a.cowriter.margin);
        let mut o = json!({"reader": "The board"});
        assert!(before_start(&mut a, Verb::ReadAsReader, ScopeKind::Document, &mut o).is_none());
        assert_eq!(reader(&a, a.session.document_id()), ("The board".to_string(), ScopeKind::Document));
        assert!(a.cowriter.reader.asking.is_none());
        // Check never takes folders from the caller.
        let mut o = json!({"folders": ["/"]});
        before_start(&mut a, Verb::CheckFacts, ScopeKind::Document, &mut o);
        assert_eq!(o["folders"], json!([]));
    }

    #[test]
    fn use_lands_one_tracked_change_by_the_notes_author_and_keep_changes_nothing() {
        const P: &str = "The studio opened in 2023 with thirty members.";
        const AI: &str = "AI · Mistral (co-writer)";
        let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
        a.session.author = "Martijn".into();
        a.session.run("text.insert", &json!({"text": P, "raw": true})).unwrap();
        let scope = edits::scope(&a.session, ScopeKind::Document).unwrap();
        let notes = [("opened in 2023", "The budget says 2024."), ("thirty members", "members.docx lists 34.")];
        let list: Vec<Edit> =
            notes.iter().map(|(q, n)| Edit { kind: EditKind::Comment, quote: (*q).into(), with: String::new(), note: (*n).into() }).collect();
        let made = edits::apply(&mut a.session, AI, &scope, &list);
        let ids: Vec<u32> = made.applied.iter().filter_map(|x| x.comment).collect();
        assert_eq!(ids.len(), 2);
        let doc = a.session.document_id();
        let outputs = vec![
            Output::Comment {
                anchor: notes[0].0.into(),
                kind: "Check".into(),
                text: notes[0].1.into(),
                options: vec!["Use 2024".into(), "Keep 2023".into()],
            },
            Output::Comment {
                anchor: notes[1].0.into(),
                kind: "Check".into(),
                text: notes[1].1.into(),
                options: vec!["Use 34".into(), "Keep thirty".into()],
            },
        ];
        remember(&mut a, doc, &outputs);
        assert_eq!(links(&a, "Check", notes[1].1), ["Use 34", "Keep thirty"]);
        // Which comment is which: by its text.
        let id_of = |a: &WordApp, text: &str| {
            *ids.iter().find(|id| a.session.doc.plain_text(wordcraft_doc::StoryRef::Part(a.session.doc.comments[id].part)).trim() == text).unwrap()
        };
        let (keep, used) = (id_of(&a, notes[0].1), id_of(&a, notes[1].1));
        let pos = a.session.doc.start_of(wordcraft_doc::StoryRef::Body);
        act(&mut a, keep, &pos, "Keep 2023");
        assert!(a.session.doc.comments[&keep].resolved);
        assert!(a.session.run("review.changes", &json!({})).unwrap().as_array().unwrap().is_empty());
        act(&mut a, used, &pos, "Use 34");
        let changes = a.session.run("review.changes", &json!({})).unwrap();
        let changes = changes.as_array().unwrap();
        // One replacement: "thirty" deleted, "34" inserted, both by the AI author.
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert!(changes.iter().all(|c| c["author"] == AI), "{changes:?}");
        assert!(a.session.doc.comments[&used].resolved);
        a.session.run("review.acceptAll", &json!({})).unwrap();
        assert_eq!(a.session.doc.plain_text(wordcraft_doc::StoryRef::Body).trim(), "The studio opened in 2023 with 34 members.");
    }

    #[test]
    fn fixed_needs_both_options_and_one_place() {
        assert_eq!(fixed("in 2023 we", &["Use 2024".into(), "Keep 2023".into()]).as_deref(), Some("in 2024 we"));
        assert_eq!(fixed("2023 or 2023", &["Use 2024".into(), "Keep 2023".into()]), None);
        assert_eq!(fixed("in 2023", &["Use 2024".into()]), None);
    }
}
