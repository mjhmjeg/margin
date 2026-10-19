//! O2 (OPTIMO.md §3): ⌘Y Anything. A 640-wide box centred over the page: one input row with the
//! scope chip, then — with nothing typed — Format the selection (chips) and Go to (headings), and
//! once you type one ranked list across the co-writer's verbs, every registry command, the
//! headings and recent files. It replaced phase 6's toolbar field and the old Search Commands
//! dialog, in both layouts.
//!
//! O3: with a co-writer set up, the empty box starts with the Co-writer group (only the verbs
//! that have landed, `Verb::available`), and typed text gets the "Ask the co-writer: “…”" row —
//! on top when nothing matches or the text reads like a sentence. An ask comes back as notes in
//! the margin, never as inserted text (rule 1).
//!
//! Everything the box lists is a row ([`Row`]), built by pure functions ([`search`], [`rows`],
//! [`results`]) so the ranking is tested without a window.

use std::collections::BTreeMap;

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::{CommandSpec, Registry};

use crate::WordApp;
use crate::mac_controls as mc;
use crate::skin::OptimoTokens;
use crate::theme::Tokens;

/// Y1: the box's width and its distance from the window's top.
pub const BOX_W: f32 = 640.0;
const BOX_TOP: f32 = 92.0;
const INPUT_H: f32 = 48.0;
pub const ROW_H: f32 = 32.0;
/// Rows visible before the list scrolls.
const VISIBLE_ROWS: f32 = 8.0;
/// How many recent commands are remembered, and how many distinct commands keep a count.
const MAX_RECENT: usize = 12;
const MAX_COUNTED: usize = 300;
/// The key that focuses the field: ⌘Y (⌥Q does too, in the Optimo layout). ⌘J is Justify (Word's
/// shortcut), and Redo is only ⇧⌘Z. macOS shortcuts match the typed letter, so on a QWERTZ keyboard
/// ⌘Y is the key labelled Y (not Undo's position, which is Z there). This constant, the Help
/// menu's accelerator and `mac_ui::shortcuts` are all that name it.
pub const SHORTCUT_LABEL: &str = "⌘Y";

/// What an ask is about: the selection captured when the field opened (with its word count),
/// else the section where the caret is ("This section"). Never the whole document by default.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Scope {
    pub words: usize,
    pub sentences: usize,
    /// The person's selection at the moment the field opened. The ask uses it even when focus
    /// moving to the field (or a key that reached the page) changed the live selection since.
    pub sel: Option<wordcraft_engine::Selection>,
}

impl Scope {
    /// The scope of `app`'s current selection, if it has text.
    pub fn capture(app: &WordApp) -> Option<Scope> {
        let text = app.session.selected_text();
        (!text.trim().is_empty()).then(|| Scope {
            words: text.split_whitespace().count(),
            sentences: sentences(&text),
            sel: Some(app.session.sel.clone()),
        })
    }
}

/// Sentences in `text`, at least 1: pieces ended by `.`, `!`, `?` or `…` that hold a word.
pub fn sentences(text: &str) -> usize {
    text.split_terminator(['.', '!', '?', '…']).filter(|s| s.chars().any(char::is_alphanumeric)).count().max(1)
}

/// The selection's part of the scope chip: "Selection: 1 sentence" (else "Whole document", for
/// callers without a section).
pub fn scope_label(scope: Option<&Scope>) -> String {
    match scope {
        Some(s) if s.sentences == 1 => tl!("Selection: 1 sentence").to_string(),
        Some(s) => crate::i18n::fmt(tl!("Selection: {n} sentences"), &[("n", &s.sentences.to_string())]),
        None => tl!("Whole document").to_string(),
    }
}

/// The field's transient state (the counts live in [`crate::mac_ui::OptimoPrefs`]).
#[derive(Default)]
pub struct CommandField {
    pub query: String,
    pub open: bool,
    /// Take keyboard focus (and select the text) on the next frame.
    pub focus: bool,
    /// Index of the highlighted row.
    pub sel: usize,
    /// The selection the ask is scoped to, captured when the field opened.
    pub scope: Option<Scope>,
    /// "This section “…”", captured when the field opened without a selection.
    pub section: String,
    scroll_y: f32,
    scroll_to: bool,
    /// The query the highlighted row belongs to (a new query highlights the top hit again).
    last_query: String,
}

/// One line of the popover.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Command {
        id: &'static str,
        label: &'static str,
        location: &'static str,
    },
    /// "Ask the co-writer: “…”" with the typed text.
    Ask(String),
    /// A co-writer verb (O3).
    Verb(wordcraft_engine::cowriter::Verb),
    /// Go to a heading; `empty`: no text before the next heading.
    Heading {
        path: wordcraft_doc::Path,
        text: String,
        level: u8,
        empty: bool,
    },
    /// A recent file (full path).
    File(String),
}

/// Recent and frequent commands, from `ui.json`.
#[derive(Clone, Copy)]
pub struct Usage<'a> {
    pub recent: &'a [String],
    pub counts: &'a BTreeMap<String, u32>,
}

impl Usage<'_> {
    /// Lower is earlier: recent commands by recency, then counted ones by count, then the rest.
    fn rank(&self, id: &str) -> u32 {
        if let Some(p) = self.recent.iter().position(|r| r == id) {
            return p as u32;
        }
        match self.counts.get(id) {
            Some(&n) if n > 0 => 1_000 + (u32::MAX / 2 - n.min(u32::MAX / 2)),
            _ => u32::MAX,
        }
    }
}

/// Record that `id` was run from the field: front of the recents, one more on its count.
pub fn record(prefs: &mut crate::mac_ui::OptimoPrefs, id: &str) {
    prefs.command_recent.retain(|r| r != id);
    prefs.command_recent.insert(0, id.to_string());
    prefs.command_recent.truncate(MAX_RECENT);
    let n = prefs.command_counts.entry(id.to_string()).or_insert(0);
    *n = n.saturating_add(1);
    // A file full of junk ids can't grow the map: drop the least used.
    while prefs.command_counts.len() > MAX_COUNTED {
        let Some(least) = prefs.command_counts.iter().min_by_key(|(_, n)| **n).map(|(k, _)| k.clone()) else { break };
        prefs.command_counts.remove(&least);
    }
}

/// Commands matching `query`, best first. Empty query: the recent and frequent ones. A command
/// matches when the query is part of its label (in the interface language or English) or, word by
/// word, of its label, menu path or id; label matches come before the others, and within each
/// group recent and frequent commands come first.
pub fn search<'a>(reg: &'a Registry, query: &str, usage: Usage<'_>) -> Vec<&'a CommandSpec> {
    let q = query.trim();
    if q.is_empty() {
        let mut used: Vec<&CommandSpec> = reg.all().iter().filter(|c| usage.rank(c.id) != u32::MAX).collect();
        used.sort_by_key(|c| usage.rank(c.id));
        used.truncate(8);
        return used;
    }
    let en = q.to_lowercase();
    let loc = crate::i18n::lowercase(q);
    let tokens: Vec<String> = loc.split_whitespace().map(str::to_string).collect();
    let mut hits: Vec<(u8, u32, u8, usize, &CommandSpec)> = Vec::new();
    for (idx, c) in reg.all().iter().enumerate() {
        let label_en = c.label.to_lowercase();
        let label_loc = crate::i18n::lowercase(tl!(c.label));
        let tier = if label_en.contains(&en) || label_loc.contains(&loc) {
            0
        } else {
            let hay = format!(
                "{} {} {} {}",
                label_en,
                label_loc,
                c.id.to_lowercase(),
                crate::i18n::lowercase(&format!("{} {}", c.location, crate::i18n::location(c.location)))
            );
            // English words and ASCII ids also match with plain lowercase (Turkish "I" → "ı").
            if tokens.iter().all(|t| hay.contains(t.as_str())) || en.split_whitespace().all(|t| hay.contains(t)) {
                1
            } else {
                continue;
            }
        };
        let prefix = u8::from(!(label_en.starts_with(&en) || label_loc.starts_with(&loc)));
        hits.push((tier, usage.rank(c.id), prefix, idx, c));
    }
    hits.sort_by_key(|h| (h.0, h.1, h.2, h.3));
    hits.into_iter().map(|h| h.4).collect()
}

/// A query that reads as a request rather than a command name: a question or four or more words.
pub fn sentence_like(query: &str) -> bool {
    let q = query.trim();
    q.ends_with('?') || q.split_whitespace().count() >= 4
}

/// The co-writer's verbs that have landed, in Y2's order.
pub fn verbs() -> Vec<wordcraft_engine::cowriter::Verb> {
    wordcraft_engine::cowriter::Verb::GROUP.into_iter().filter(|v| v.available()).collect()
}

/// Typed: the matching verbs, then `rest` (the other matches). With a co-writer (`assistant`),
/// the "Ask the co-writer" row comes last, or first when nothing else matches or the query reads
/// as a sentence.
pub fn rows(verbs: &[wordcraft_engine::cowriter::Verb], query: &str, rest: Vec<Row>, assistant: bool) -> Vec<Row> {
    let q = query.trim();
    let lq = crate::i18n::lowercase(q);
    let mut out: Vec<Row> = verbs
        .iter()
        .filter(|v| !lq.is_empty() && (crate::i18n::lowercase(tl!(v.label())).contains(&lq) || v.label().to_lowercase().contains(&q.to_lowercase())))
        .map(|v| Row::Verb(*v))
        .collect();
    out.extend(rest);
    if !assistant || q.is_empty() {
        return out;
    }
    let ask = Row::Ask(q.to_string());
    if out.is_empty() || sentence_like(q) {
        out.insert(0, ask);
    } else {
        out.push(ask);
    }
    out
}

/// Y2 / typing: what the box lists. Nothing typed: the headings (Go to; the Co-writer group is
/// drawn above them). Typed: matching verbs, the matching commands (recent and frequent first, as
/// [`search`] ranks them), then matching headings, then matching recent files, and the ask row.
// ponytail: groups in a fixed order, not one score; mix them when people miss headings.
pub fn results(hits: &[&CommandSpec], query: &str, headings: &[Row], recent: &[String], assistant: bool) -> Vec<Row> {
    let q = crate::i18n::lowercase(query.trim());
    if q.is_empty() {
        return headings.to_vec();
    }
    let mut rest: Vec<Row> = hits.iter().map(|c| Row::Command { id: c.id, label: c.label, location: c.location }).collect();
    rest.extend(headings.iter().filter(|h| matches!(h, Row::Heading { text, .. } if crate::i18n::lowercase(text).contains(&q))).cloned());
    rest.extend(recent.iter().filter(|p| crate::i18n::lowercase(file_name(p)).contains(&q)).map(|p| Row::File(p.clone())));
    rows(&if assistant { verbs() } else { Vec::new() }, query, rest, assistant)
}

fn file_name(path: &str) -> &str {
    std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path)
}

/// The document's headings as Go to rows, each marked empty when no text follows it before the
/// next heading.
pub fn heading_rows(app: &WordApp) -> Vec<Row> {
    let (items, _) = crate::panes::nav_items(app);
    let doc = &app.session.doc;
    let body = wordcraft_doc::StoryRef::Body;
    let paras = doc.para_paths(body);
    let top = |p: &wordcraft_doc::Path| p.0.first().copied().unwrap_or(0);
    let mut out = Vec::with_capacity(items.len());
    for (i, (path, level, text)) in items.iter().enumerate() {
        let from = top(path);
        let to = items.get(i + 1).map(|n| top(&n.0));
        let empty = !paras
            .iter()
            .filter(|p| top(p) > from && to.is_none_or(|t| top(p) < t))
            .any(|p| doc.para(body, p).is_some_and(|para| !para.text.trim().is_empty()));
        out.push(Row::Heading { path: path.clone(), text: text.clone(), level: *level, empty });
    }
    out
}

/// Send an ask to the co-writer: notes in the margin. Scoped to the selection when the field was
/// opened on one, else the section at the caret.
pub fn submit_ask(app: &mut WordApp, instruction: &str, scope: &Scope) -> Result<(), String> {
    let kind = scope_kind(app, scope);
    crate::cowriter::ask(app, instruction, kind).map(|_| ())
}

/// What an ask from the box is about, as its row says: "Selection: 1 sentence" or "This section “…”".
pub fn ask_scope_label(f: &CommandField) -> String {
    match &f.scope {
        Some(_) => scope_label(f.scope.as_ref()),
        None if f.section.is_empty() => tl!("This section").to_string(),
        None => f.section.clone(),
    }
}

/// Selection when one was captured (put back as the live selection first), else This section.
fn scope_kind(app: &mut WordApp, scope: &Scope) -> wordcraft_engine::assistant_edits::ScopeKind {
    use wordcraft_engine::assistant_edits::ScopeKind;
    match &scope.sel {
        Some(sel) if scope.words > 0 => {
            let (a, b) = (app.session.doc.clamp(&sel.anchor), app.session.doc.clamp(&sel.focus));
            if a != b {
                app.session.sel = wordcraft_engine::Selection { anchor: a, focus: b };
                return ScopeKind::Selection;
            }
            ScopeKind::Section
        }
        None if scope.words > 0 && !app.session.sel.is_collapsed() => ScopeKind::Selection,
        _ => ScopeKind::Section,
    }
}

/// The scope line's text for "This section" at the caret: with the section's heading.
fn section_label(app: &WordApp) -> String {
    use wordcraft_engine::assistant_edits::{self as edits, ScopeKind};
    match edits::scope(&app.session, ScopeKind::Section).ok().and_then(|s| s.title) {
        Some(t) => format!("{} “{}”", tl!("This section"), crate::review_panel::clip(&t, 40)),
        None => tl!("This section").to_string(),
    }
}

/// Open ⌘Y Anything (⌘Y, the pill, Help › Search Commands…, Alt+Q on Windows/Linux), in either
/// layout.
pub fn open(app: &mut WordApp, query: &str) {
    let f = &mut app.command_field;
    f.query = query.to_string();
    f.open = true;
    f.focus = true;
    f.sel = 0;
    f.scroll_y = 0.0;
    capture_scope(app);
}

/// Take the selection (or the section's name) as the ask's scope, at the moment the field opens.
fn capture_scope(app: &mut WordApp) {
    app.command_field.scope = Scope::capture(app);
    app.command_field.section = if app.command_field.scope.is_none() { section_label(app) } else { String::new() };
}

/// The context menu's "Ask …": ⌘Y, scoped to the selection, ready for the question.
pub fn open_ask(app: &mut WordApp) {
    open(app, "");
}

fn close(app: &mut WordApp, ctx: &egui::Context) {
    let f = &mut app.command_field;
    f.open = false;
    f.focus = false;
    f.query.clear();
    f.scope = None;
    ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("command_field_edit")));
    app.canvas.want_focus = true;
}

fn run_row(app: &mut WordApp, ctx: &egui::Context, row: &Row) {
    match row {
        Row::Command { id, .. } => {
            record(&mut app.ui.optimo, id);
            let _ = app.run(id, json!({}));
        }
        Row::Ask(text) => ask(app, text),
        Row::Verb(v) => {
            let scope = app.command_field.scope.clone().unwrap_or_default();
            let kind = scope_kind(app, &scope);
            if let Err(e) = crate::cowriter::start(app, *v, kind, json!({})) {
                app.status(e);
            }
        }
        Row::Heading { path, .. } => {
            let _ = app.run("caret.set", json!({"pos": wordcraft_doc::Pos { story: wordcraft_doc::StoryRef::Body, path: path.clone(), off: 0 }}));
            app.canvas.nav_to_caret = true;
            app.canvas.scroll_to_caret = false;
        }
        Row::File(path) => {
            let _ = app.run("file.open", json!({"path": path}));
        }
    }
    close(app, ctx);
}

fn ask(app: &mut WordApp, instruction: &str) {
    let scope = app.command_field.scope.clone().unwrap_or_default();
    if let Err(e) = submit_ask(app, instruction, &scope) {
        app.status(e);
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing

/// Y2: the Format the selection chips: label, command, params. "More format…" opens the
/// Format inspector instead.
const CHIPS: [(&str, &str, &str); 6] = [
    ("B", "format.bold", "{}"),
    ("I", "format.italic", "{}"),
    ("Heading 2", "para.style", r#"{"style": "Heading2"}"#),
    ("Quote", "para.style", r#"{"style": "Quote"}"#),
    ("Link…", "insert.link", "{}"),
    ("Comment", "review.newComment", "{}"),
];
const MORE_FORMAT: &str = "More format…";

/// What a click in the box asked for.
enum Pick {
    Row(usize),
    Chip(usize),
    MoreFormat,
    Verb(wordcraft_engine::cowriter::Verb),
    /// Back to the field (the Ask row of the group: type the question there).
    Focus,
}

/// The ⌘Y box while it is open: centred over the window, 92 below its top (Y1).
pub fn show_box(app: &mut WordApp, ctx: &egui::Context) {
    if !app.command_field.open {
        return;
    }
    let t = Tokens::get(ctx);
    let m = OptimoTokens::get(ctx);
    let assistant = crate::review_panel::assistant_enabled();
    let edit_id = egui::Id::new("command_field_edit");

    // Keys first, so the text field doesn't also act on them.
    let focused = ctx.memory(|mem| mem.has_focus(edit_id));
    let pending = if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        Some(Key::Escape)
    } else if focused {
        take_keys(ctx)
    } else {
        None
    };

    let f = &mut app.command_field;
    if f.query != f.last_query {
        f.last_query = f.query.clone();
        f.sel = 0;
        f.scroll_y = 0.0;
        f.scroll_to = true;
    }
    let reg = app.session.registry.clone();
    let usage = Usage { recent: &app.ui.optimo.command_recent, counts: &app.ui.optimo.command_counts };
    let typed = !app.command_field.query.trim().is_empty();
    let hits = if typed { search(&reg, &app.command_field.query, usage) } else { Vec::new() };
    let headings = heading_rows(app);
    let list = results(&hits, &app.command_field.query, &headings, &app.ui.recent, assistant);
    let f = &mut app.command_field;
    if f.sel >= list.len() {
        f.sel = list.len().saturating_sub(1);
    }
    let mut pick: Option<Pick> = None;
    match pending {
        Some(Key::Down) if !list.is_empty() => {
            f.sel = (f.sel + 1) % list.len();
            f.scroll_to = true;
        }
        Some(Key::Up) if !list.is_empty() => {
            f.sel = (f.sel + list.len() - 1) % list.len();
            f.scroll_to = true;
        }
        Some(Key::Enter) if !list.is_empty() => pick = Some(Pick::Row(f.sel)),
        Some(Key::Escape) => {
            close(app, ctx);
            return;
        }
        _ => {}
    }

    let screen = ctx.content_rect();
    let w = BOX_W.min(screen.width() - 32.0).max(200.0);
    let area = egui::Area::new(egui::Id::new("anything_box"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(screen.center().x - w / 2.0, screen.min.y + BOX_TOP))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(m.popover_bg)
                .stroke(Stroke::new(1.0, m.popover_border))
                .corner_radius(12)
                .shadow(egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(41) })
                .show(ui, |ui| {
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    input_row(app, ui, w, &t, edit_id);
                    let r = ui.available_rect_before_wrap();
                    ui.painter().hline(r.x_range(), r.min.y, Stroke::new(1.0, t.border));
                    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                        if !typed {
                            if assistant && let Some(p) = cowriter_group(ui, &t) {
                                pick = Some(p);
                            }
                            if let Some(p) = format_chips(ui, &t, &m) {
                                pick = Some(p);
                            }
                            group_title(ui, tl!("Go to"), &t);
                        }
                        if let Some(i) = list_ui(app, ui, &list, &t) {
                            pick = Some(Pick::Row(i));
                        }
                    });
                    footer(ui, reg.all().len(), &t);
                });
        });
    // A press outside the box closes it.
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
    match pick {
        Some(Pick::Row(i)) => {
            if let Some(row) = list.get(i) {
                run_row(app, ctx, row);
            }
        }
        Some(Pick::Chip(i)) => {
            if let Some((_, id, params)) = CHIPS.get(i) {
                record(&mut app.ui.optimo, id);
                let _ = app.run(id, serde_json::from_str(params).unwrap_or_else(|_| json!({})));
            }
            close(app, ctx);
        }
        Some(Pick::Verb(v)) => {
            run_row(app, ctx, &Row::Verb(v));
        }
        Some(Pick::Focus) => app.command_field.focus = true,
        Some(Pick::MoreFormat) => {
            close(app, ctx);
            if !app.ui.optimo.inspector {
                crate::mac_ui::toggle_panel(app, crate::mac_ui::Panel::Format);
            }
        }
        None if outside => close(app, ctx),
        None => {}
    }
}

/// Y1: the 48 pt input row: search icon, the field, the scope chip on the right.
fn input_row(app: &mut WordApp, ui: &mut Ui, w: f32, t: &Tokens, edit_id: egui::Id) {
    let (r, _) = ui.allocate_exact_size(vec2(w, INPUT_H), Sense::hover());
    let font = mc::font();
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 22.0, r.center().y), vec2(17.0, 17.0)), "search", t.text_dim, t.text_dim);
    // What an ask from here is about: the selection, else the section at the caret (O3).
    let chip = ask_scope_label(&app.command_field);
    let g = ui.painter().layout_no_wrap(chip.clone(), font.clone(), t.text_dim);
    let chip_r = Rect::from_min_max(pos2(r.max.x - 14.0 - g.size().x - 20.0, r.center().y - 11.0), pos2(r.max.x - 14.0, r.center().y + 11.0));
    ui.painter().rect_filled(chip_r, 10.0, t.checked);
    ui.painter().galley(chip_r.center() - g.size() / 2.0, g, t.text_dim);
    let placeholder = tl!("Do, find, format, go to — or ask your co-writer");
    let edit_rect = Rect::from_min_max(pos2(r.min.x + 42.0, r.min.y + 8.0), pos2(chip_r.min.x - 10.0, r.max.y - 8.0));
    let f = &mut app.command_field;
    let te = ui.put(
        edit_rect,
        egui::TextEdit::singleline(&mut f.query)
            .id(edit_id)
            .frame(egui::Frame::NONE)
            .vertical_align(egui::Align::Center)
            .font(egui::FontId::proportional(crate::skin::UI_SIZE + 2.0))
            .text_color(t.text)
            .hint_text(egui::RichText::new(placeholder).color(t.text_dim))
            .lock_focus(true)
            .desired_width(edit_rect.width()),
    );
    if f.focus {
        te.request_focus();
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), edit_id) {
            let end = egui::text::CCursor::new(f.query.chars().count());
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ui.ctx(), edit_id);
        }
        f.focus = false;
    }
    te.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, placeholder));
}

/// A group's title, 28 high, in `text_dim`.
fn group_title(ui: &mut Ui, title: &str, t: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, title, mc::font(), t.text_dim);
}

/// Y2 "Co-writer": the verbs that have landed, then Ask (type above). Rows 32 high: the name
/// (168 wide), a one-line description in `text_dim`, the shortcut on the right.
fn cowriter_group(ui: &mut Ui, t: &Tokens) -> Option<Pick> {
    use wordcraft_engine::cowriter::Verb;
    group_title(ui, tl!("Co-writer"), t);
    let mut pick = None;
    for v in verbs().into_iter().chain([Verb::Ask]) {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 6.0, t.checked);
        }
        let font = mc::font();
        let m = crate::skin::OptimoTokens::get(ui.ctx());
        crate::icons::paint(
            ui.painter(),
            Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(15.0, 15.0)),
            "assistant",
            m.assistant,
            m.assistant,
        );
        let name = if v == Verb::Ask { tl!("Ask the co-writer…") } else { tl!(v.label()) };
        let mut job = egui::text::LayoutJob::simple_singleline(name.to_string(), font.clone(), t.text);
        job.wrap = egui::text::TextWrapping::truncate_at_width(168.0 - 34.0);
        let g = ui.painter().layout_job(job);
        ui.painter().galley(pos2(r.min.x + 34.0, r.center().y - g.size().y / 2.0), g, t.text);
        let desc = if v == Verb::Ask { tl!("Type your question above · notes in the margin") } else { tl!(v.description()) };
        let shortcut = crate::widgets::key_text(v.shortcut());
        let sc_w = if shortcut.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(shortcut.clone(), font.clone(), t.text_dim).size().x + 12.0 };
        if !shortcut.is_empty() {
            ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, &shortcut, font.clone(), t.text_dim);
        }
        let mut job = egui::text::LayoutJob::simple_singleline(desc.to_string(), font, t.text_dim);
        job.wrap = egui::text::TextWrapping::truncate_at_width((r.width() - 178.0 - sc_w).max(40.0));
        let g = ui.painter().layout_job(job);
        ui.painter().galley(pos2(r.min.x + 178.0, r.center().y - g.size().y / 2.0), g, t.text_dim);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
        if resp.clicked() {
            pick = Some(if v == Verb::Ask { Pick::Focus } else { Pick::Verb(v) });
        }
    }
    ui.add_space(4.0);
    pick
}

/// Y2 "Format the selection": chips 26 high, radius 6, `field_bg`, `border_strong`.
fn format_chips(ui: &mut Ui, t: &Tokens, m: &OptimoTokens) -> Option<Pick> {
    group_title(ui, tl!("Format the selection"), t);
    let mut pick = None;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let labels = CHIPS.iter().map(|c| c.0).chain([MORE_FORMAT]);
        for (i, label) in labels.enumerate() {
            let mut job = egui::text::LayoutJob::default();
            let font = if label == "B" { crate::theme::semibold(crate::skin::UI_SIZE) } else { mc::font() };
            job.append(tl!(label), 0.0, egui::TextFormat { font_id: font, color: t.text, italics: label == "I", ..Default::default() });
            let g = ui.painter().layout_job(job);
            let (r, resp) = ui.allocate_exact_size(vec2((g.size().x + 20.0).max(28.0), 26.0), Sense::click());
            let fill = if resp.hovered() { t.hover } else { m.field_bg };
            ui.painter().rect(r, 6.0, fill, Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
            ui.painter().galley(r.center() - g.size() / 2.0, g, t.text);
            let name = match label {
                "B" => tl!("Bold"),
                "I" => tl!("Italic"),
                other => tl!(other),
            };
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
            if resp.clicked() {
                pick = Some(if label == MORE_FORMAT { Pick::MoreFormat } else { Pick::Chip(i) });
            }
        }
    });
    ui.add_space(6.0);
    pick
}

/// The rows (scrolling past 8). Returns the row clicked.
fn list_ui(app: &mut WordApp, ui: &mut Ui, list: &[Row], t: &Tokens) -> Option<usize> {
    let mut clicked = None;
    if list.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        let text = if app.command_field.query.trim().is_empty() {
            tl!("No headings yet").to_string()
        } else {
            crate::i18n::fmt(tl!("Nothing matches “{query}”"), &[("query", app.command_field.query.trim())])
        };
        ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, text, mc::font(), t.text_dim);
        return None;
    }
    let visible = (list.len() as f32).min(VISIBLE_ROWS);
    let view_h = visible * ROW_H;
    let sel = app.command_field.sel;
    let mut area = egui::ScrollArea::vertical().max_height(view_h).auto_shrink([false, true]).id_salt("command_field_rows");
    if std::mem::take(&mut app.command_field.scroll_to) {
        let top = sel as f32 * ROW_H;
        let mut y = app.command_field.scroll_y;
        if top < y {
            y = top;
        } else if top + ROW_H > y + view_h {
            y = top + ROW_H - view_h;
        }
        area = area.vertical_scroll_offset(y);
    }
    let out = area.show_rows(ui, ROW_H, list.len(), |ui, range| {
        for i in range {
            let Some(row) = list.get(i) else { continue };
            if paint_row(app, ui, row, i == sel, t) {
                clicked = Some(i);
            }
        }
    });
    app.command_field.scroll_y = out.state.offset.y;
    clicked
}

/// Y2 footer: "Type to search 600 commands · ↩ run · esc close".
fn footer(ui: &mut Ui, commands: usize, t: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.min.y, Stroke::new(1.0, t.border));
    let text = format!(
        "{} · ↩ {} · esc {}",
        crate::i18n::fmt(tl!("Type to search {n} commands"), &[("n", &commands.to_string())]),
        tl!("run"),
        tl!("close")
    );
    ui.painter().text(pos2(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, text, mc::font(), t.text_dim);
}

/// One row, 32 high: name, then its menu path (or kind) in `text_dim`, the shortcut on the right.
/// True when clicked.
fn paint_row(app: &WordApp, ui: &mut Ui, row: &Row, highlighted: bool, t: &Tokens) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if highlighted || resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.checked);
    }
    let font = mc::font();
    let (name, detail, shortcut, enabled) = match row {
        Row::Command { id, label, location } => {
            let enabled = app.session.registry.get(id).is_none_or(|s| (s.enabled)(&app.session).is_none());
            (tl!(label).to_string(), crate::i18n::location(location), crate::widgets::shortcut_text(app, id), enabled)
        }
        Row::Heading { text, level, empty, .. } => {
            let indent = "   ".repeat(usize::from(level.saturating_sub(1)).min(5));
            let kind = if *empty { tl!("Heading · empty") } else { tl!("Heading") };
            (format!("{indent}{text}"), kind.to_string(), String::new(), true)
        }
        Row::File(path) => (file_name(path).to_string(), tl!("Recent file").to_string(), String::new(), true),
        Row::Ask(text) => {
            (crate::i18n::fmt(tl!("Ask the co-writer: “{text}”"), &[("text", text)]), tl!("Notes in the margin").to_string(), String::new(), true)
        }
        Row::Verb(v) => (tl!(v.label()).to_string(), tl!(v.description()).to_string(), crate::widgets::key_text(v.shortcut()), true),
    };
    let ink = if enabled { t.text } else { t.text_dim };
    let sc_w = if shortcut.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(shortcut.clone(), font.clone(), t.text_dim).size().x + 12.0 };
    if !shortcut.is_empty() {
        ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, &shortcut, font.clone(), t.text_dim);
    }
    let avail = r.width() - 20.0 - sc_w;
    let mut job = egui::text::LayoutJob::simple_singleline(name.clone(), font.clone(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(avail.min(360.0));
    let g = ui.painter().layout_job(job);
    let name_w = g.size().x;
    ui.painter().galley(pos2(r.min.x + 10.0, r.center().y - g.size().y / 2.0), g, ink);
    let detail_w = avail - name_w - 12.0;
    if detail_w > 40.0 && !detail.is_empty() {
        let mut job = egui::text::LayoutJob::simple_singleline(detail, font, t.text_dim);
        job.wrap = egui::text::TextWrapping::truncate_at_width(detail_w);
        let g = ui.painter().layout_job(job);
        ui.painter().galley(pos2(r.min.x + 10.0 + name_w + 12.0, r.center().y - g.size().y / 2.0), g, t.text_dim);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name.trim()));
    resp.clicked()
}

enum Key {
    Up,
    Down,
    Enter,
    Escape,
}

/// Take the box's navigation keys out of this frame's input.
fn take_keys(ctx: &egui::Context) -> Option<Key> {
    ctx.input_mut(|i| {
        let none = egui::Modifiers::NONE;
        if i.consume_key(none, egui::Key::ArrowDown) {
            Some(Key::Down)
        } else if i.consume_key(none, egui::Key::ArrowUp) {
            Some(Key::Up)
        } else if i.consume_key(none, egui::Key::Enter) {
            Some(Key::Enter)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    fn session() -> wordcraft_engine::Session {
        wordcraft_engine::Session::new(wordcraft_doc::Document::new())
    }

    const NO_USE: Usage<'static> = Usage { recent: &[], counts: &BTreeMap::new() };

    /// The done-when from HANDOFF §6: every registry command is reachable by typing its label.
    #[test]
    fn every_registry_command_is_found_by_its_own_label() {
        let s = session();
        for c in s.registry.all() {
            let hits = search(&s.registry, c.label, NO_USE);
            assert!(hits.iter().any(|h| h.id == c.id), "{} ({:?}) is not in the results for its own label", c.id, c.label);
        }
    }

    #[test]
    fn id_menu_path_and_words_match_too() {
        let s = session();
        let ids = |q: &str| search(&s.registry, q, NO_USE).iter().map(|c| c.id).collect::<Vec<_>>();
        assert!(ids("FORMAT.ITALIC").contains(&"format.italic"));
        assert!(ids("home font").contains(&"format.italic"), "menu path words match");
        assert!(ids("insert table").contains(&"insert.table"));
        assert!(ids("zzzqq").is_empty());
        assert!(search(&s.registry, "   ", NO_USE).is_empty(), "no history, nothing to suggest");
    }

    #[test]
    fn label_matches_come_first_and_used_commands_before_unused() {
        let s = session();
        let hits = search(&s.registry, "bold", NO_USE);
        assert_eq!(hits.first().map(|c| c.id), Some("format.bold"), "exact label first");
        let plain = search(&s.registry, "under", NO_USE);
        let second = plain.get(1).map(|c| c.id).unwrap_or("");
        assert!(!second.is_empty());
        let recent = vec![second.to_string()];
        let used = search(&s.registry, "under", Usage { recent: &recent, counts: &BTreeMap::new() });
        assert_eq!(used.first().map(|c| c.id), Some(second), "a recent command leads its group");
    }

    #[test]
    fn empty_query_lists_recent_then_frequent() {
        let s = session();
        let mut counts = BTreeMap::new();
        counts.insert("format.italic".to_string(), 5);
        counts.insert("format.underline".to_string(), 2);
        let recent = vec!["format.bold".to_string()];
        let ids: Vec<_> = search(&s.registry, "", Usage { recent: &recent, counts: &counts }).iter().map(|c| c.id).collect();
        assert_eq!(ids, ["format.bold", "format.italic", "format.underline"]);
    }

    #[test]
    fn counts_are_recorded_capped_and_survive_junk() {
        let mut p = crate::mac_ui::OptimoPrefs::default();
        for _ in 0..3 {
            record(&mut p, "format.bold");
        }
        record(&mut p, "format.italic");
        assert_eq!(p.command_counts["format.bold"], 3);
        assert_eq!(p.command_recent, ["format.italic", "format.bold"]);
        p.command_counts.insert("x".into(), u32::MAX);
        record(&mut p, "x");
        assert_eq!(p.command_counts["x"], u32::MAX, "saturates");
        for i in 0..400 {
            record(&mut p, &format!("id{i}"));
        }
        assert!(p.command_counts.len() <= MAX_COUNTED && p.command_recent.len() <= MAX_RECENT);
    }

    /// The ask row: absent while no co-writer is set up; last, or first when nothing matches or
    /// the query reads as a sentence. Only verbs that have landed are listed.
    #[test]
    fn the_ask_row_follows_the_co_writer_switch() {
        let s = session();
        let bold = search(&s.registry, "bold", NO_USE);
        let cmds = |h: &[&CommandSpec]| h.iter().map(|c| Row::Command { id: c.id, label: c.label, location: c.location }).collect::<Vec<_>>();
        assert!(!crate::review_panel::assistant_enabled(), "off until Settings › Co-writer");
        let off = rows(&[], "bold", cmds(&bold), false);
        assert!(off.iter().all(|r| matches!(r, Row::Command { .. })), "no ask row while off");
        let on = rows(&[], "bold", cmds(&bold), true);
        assert!(matches!(on.last(), Some(Row::Ask(t)) if t == "bold"), "ask row is last");
        let sentence = rows(&[], "make the second paragraph shorter", cmds(&bold), true);
        assert!(matches!(sentence.first(), Some(Row::Ask(_))), "a sentence puts the ask row first");
        assert_eq!(rows(&[], "zzz", Vec::new(), true), [Row::Ask("zzz".into())], "nothing matches: only the ask row");
        assert!(rows(&[], "  ", Vec::new(), true).is_empty(), "nothing typed, no ask");
        // A verb matches by its name, before the commands.
        use wordcraft_engine::cowriter::Verb;
        let v = rows(&[Verb::Alternatives, Verb::FormatForMe], "format", cmds(&bold), true);
        assert_eq!(v.first(), Some(&Row::Verb(Verb::FormatForMe)));
        assert!(verbs().iter().all(|v| v.available()), "only landed verbs");
    }

    #[test]
    fn asking_without_a_co_writer_is_an_error_not_a_panic() {
        let mut app = WordApp::new(session(), crate::Services::default());
        let r = submit_ask(&mut app, "Shorten this.", &Scope { words: 3, ..Default::default() });
        assert!(r.as_ref().is_err_and(|e| e.contains("Let a co-writer work in Optimo")), "{r:?}");
        ask(&mut app, "Shorten this.");
        assert!(app.status_msg.as_ref().is_some_and(|(m, _)| m.contains("Let a co-writer work in Optimo")));
        assert!(!app.cowriter.working());
    }

    #[test]
    fn sentences_are_told_from_command_names() {
        assert!(sentence_like("why is this paragraph so long?"));
        assert!(sentence_like("make this a bit shorter"));
        assert!(!sentence_like("insert table"));
        assert!(!sentence_like("bold"));
    }

    fn harness() -> egui_kittest::Harness<'static, WordApp> {
        let mut app = WordApp::new(session(), crate::Services::default());
        app.integrated_titlebar = true;
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        );
        h.run_steps(6);
        h
    }

    /// Help › Search Commands… focuses the field; type, ↩ runs the top hit and counts it.
    #[test]
    fn help_menu_focuses_the_field_and_enter_runs_the_top_hit() {
        let mut h = harness();
        let act = crate::mac_menu::find("Help/Search Commands…").expect("menu item");
        crate::mac_menu::perform(h.state_mut(), &act);
        h.run_steps(3);
        assert!(h.state().command_field.open && h.state().dialog.is_none(), "the field, not the dialog");
        assert!(h.ctx.memory(|m| m.has_focus(egui::Id::new("command_field_edit"))), "the field has keyboard focus");
        h.event(egui::Event::Text("show ruler".into()));
        h.run_steps(3);
        let ruler = h.state().session.view.ruler;
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        assert_ne!(h.state().session.view.ruler, ruler, "↩ ran View › Show Ruler");
        assert!(!h.state().command_field.open && h.state().command_field.query.is_empty());
        assert_eq!(h.state().ui.optimo.command_counts.get("view.ruler"), Some(&1));
        assert_eq!(h.state().ui.optimo.command_recent, ["view.ruler"]);
    }

    /// ⌘Y and ⌥Q focus the field; Redo is only ⇧⌘Z (⌘Y no longer redoes). Shortcuts match the
    /// typed letter, so on QWERTZ keyboards ⌘Y is the key labelled Y.
    #[test]
    fn cmd_y_and_alt_q_focus_the_field_and_redo_is_shift_cmd_z() {
        let cmd = egui::Modifiers { mac_cmd: true, command: true, ..Default::default() };
        let cmd_shift = egui::Modifiers { shift: true, ..cmd };
        let alt = egui::Modifiers { alt: true, ..Default::default() };
        let mut h = harness();
        let text = |h: &egui_kittest::Harness<'_, WordApp>| h.state().session.doc.plain_text(wordcraft_doc::StoryRef::Body);
        h.state_mut().run("text.insert", json!({"text": "Kilns"})).unwrap();
        h.state_mut().run("edit.undo", json!({})).unwrap();
        assert_eq!(text(&h), "");
        h.key_press_modifiers(cmd, egui::Key::Y);
        h.run_steps(3);
        assert!(h.state().command_field.open && h.state().dialog.is_none(), "⌘Y opens the field");
        assert!(h.ctx.memory(|m| m.has_focus(egui::Id::new("command_field_edit"))));
        assert_eq!(text(&h), "", "⌘Y does not redo");
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        assert!(!h.state().command_field.open);
        h.key_press_modifiers(alt, egui::Key::Q);
        h.run_steps(3);
        assert!(h.state().command_field.open && h.state().dialog.is_none(), "⌥Q opens the field in the Optimo layout");
        assert!(h.state().command_field.query.is_empty(), "no stray character");
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        h.key_press_modifiers(cmd_shift, egui::Key::Z);
        h.run_steps(3);
        assert_eq!(text(&h), "Kilns", "⇧⌘Z redoes");
        assert_eq!(SHORTCUT_LABEL, "⌘Y");
    }

    #[test]
    fn escape_closes_and_arrows_move() {
        let mut h = harness();
        h.state_mut().run("ui.commandField", json!({"query": "under"})).unwrap();
        h.run_steps(3);
        assert!(h.state().command_field.open);
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        assert_eq!(h.state().command_field.sel, 1);
        h.key_press(egui::Key::ArrowUp);
        h.run_steps(2);
        assert_eq!(h.state().command_field.sel, 0);
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
        assert!(!h.state().command_field.open);
    }

    /// Search Commands is ⌘Y Anything in both layouts; the old dialog is gone (O2).
    #[test]
    fn both_layouts_open_the_box() {
        let mut h = harness();
        h.state_mut().ui.optimo.classic_ribbon = true;
        h.state_mut().run("ui.dialog", json!({"name": "commands"})).unwrap();
        assert!(h.state().dialog.is_none() && h.state().command_field.open);
        h.run_steps(3);
        assert!(h.query_by_label("Do, find, format, go to — or ask your co-writer").is_some(), "the box shows in Classic too");
    }

    /// Y2: nothing typed shows the Format chips and the headings (empty sections marked); a chip
    /// runs on the selection and closes the box; the scope chip names the selection.
    #[test]
    fn empty_query_shows_chips_and_headings() {
        let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_engine::sample::sample_document()), crate::Services::default());
        app.integrated_titlebar = true;
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        );
        h.run_steps(4);
        let rows = heading_rows(h.state());
        assert!(rows.iter().any(|r| matches!(r, Row::Heading { text, empty: false, .. } if text == "Why share a studio?")), "{rows:?}");
        assert!(!rows.iter().any(|r| matches!(r, Row::Heading { empty: true, .. })), "the sample has no empty section");
        // A heading straight after another: the first one's section is empty.
        let mut a = WordApp::new(session(), crate::Services::default());
        for (text, style) in [("Costs", "Heading1"), ("Rules", "Heading1"), ("Be kind.", "Normal")] {
            a.run("text.insert", json!({"text": text})).unwrap();
            a.run("para.style", json!({"style": style})).unwrap();
            a.run("text.newParagraph", json!({})).unwrap();
        }
        let empty: Vec<_> = heading_rows(&a)
            .into_iter()
            .map(|r| if let Row::Heading { text, empty, .. } = r { (text, empty) } else { (String::new(), false) })
            .collect();
        assert_eq!(empty, [("Costs".to_string(), true), ("Rules".to_string(), false)]);
        h.state_mut().run("select.text", json!({"text": "Every Thursday evening"})).unwrap();
        h.key_press_modifiers(egui::Modifiers { mac_cmd: true, command: true, ..Default::default() }, egui::Key::Y);
        h.run_steps(3);
        assert!(!crate::mini_toolbar::visible(h.state()), "the mini toolbar stays out of the box's way");
        assert_eq!(scope_label(h.state().command_field.scope.as_ref()), "Selection: 1 sentence");
        assert_eq!(ask_scope_label(&h.state().command_field), "Selection: 1 sentence", "the chip");
        assert!(h.query_by_label("Why share a studio?").is_some(), "Go to lists the headings");
        h.get_by_label("Heading 2").click();
        h.run_steps(3);
        assert!(!h.state().command_field.open, "a chip closes the box");
        let st = h.state_mut().session.run("format.state", &json!({})).unwrap();
        assert_eq!(st.get("style"), Some(&json!("Heading2")), "the chip styled the selection's paragraph: {st}");
    }

    #[test]
    fn typed_query_lists_commands_then_headings_then_files() {
        let s = session();
        let heads = [
            Row::Heading { path: wordcraft_doc::Path::top(0), text: "Studio rules".into(), level: 1, empty: false },
            Row::Heading { path: wordcraft_doc::Path::top(3), text: "Costs".into(), level: 1, empty: true },
        ];
        let recent = ["/tmp/Studio notes.docx".to_string(), "/tmp/other.docx".to_string()];
        assert_eq!(results(&[], "", &heads, &recent, true), heads.to_vec(), "nothing typed: Go to");
        let hits = search(&s.registry, "studio", NO_USE);
        let out = results(&hits, "studio", &heads, &recent, false);
        assert!(matches!(out.get(out.len() - 2), Some(Row::Heading { text, .. }) if text == "Studio rules"), "{out:?}");
        assert_eq!(out.last(), Some(&Row::File("/tmp/Studio notes.docx".into())));
        assert!(!out.iter().any(|r| matches!(r, Row::Ask(_))), "no ask row without a co-writer");
        assert_eq!(sentences("One. Two! Three?"), 3);
        assert_eq!(sentences("no stop at the end"), 1);
        assert_eq!(sentences("Etc. …"), 1);
        assert_eq!(scope_label(None), "Whole document");
    }

    /// ⇧⌘Y is Repeat, not ⌘Y Anything (egui would match ⌘Y with Shift held).
    #[cfg(target_os = "macos")]
    #[test]
    fn shift_cmd_y_repeats() {
        let mut h = harness();
        h.state_mut().run("text.insert", json!({"text": "ab"})).unwrap();
        h.state_mut().canvas.want_focus = true;
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers { shift: true, mac_cmd: true, command: true, ..Default::default() }, egui::Key::Y);
        h.run_steps(3);
        assert!(!h.state().command_field.open, "⇧⌘Y does not open the box");
        assert_eq!(h.state().session.doc.plain_text(wordcraft_doc::StoryRef::Body), "abab");
    }

    /// Ordinary Turkish queries find dotted-I labels; English labels and ASCII command ids keep
    /// matching case-insensitively in every interface language (moved from the old dialog).
    #[test]
    fn search_respects_turkish_i_pairs_and_keeps_canonical_matches() {
        let s = session();
        for (language, query, id) in [
            ("tr", "italik", "format.italic"),
            ("tr", "İTALİK", "format.italic"),
            ("tr", "içindekiler", "references.toc"),
            ("tr", "INSERT.TABLE", "insert.table"),
            ("tr", "FORMAT.ITALIC", "format.italic"),
            ("tr", "TABLE OF CONTENTS", "references.toc"),
            ("en", "ITALIC", "format.italic"),
            ("en", "INSERT.TABLE", "insert.table"),
        ] {
            crate::i18n::set_current(crate::i18n::Lang::from_pref(language));
            let ids: Vec<_> = search(&s.registry, query, NO_USE).iter().map(|c| c.id).collect();
            crate::i18n::set_current(crate::i18n::Lang::EN);
            assert!(ids.contains(&id), "{language}: {query} → {ids:?}");
        }
    }

    /// The counts come back after a restart, and junk in `ui.json` reads as no history.
    #[test]
    fn usage_survives_a_restart_and_junk() {
        let mut ui = crate::UiState::default();
        record(&mut ui.optimo, "format.bold");
        let bytes = serde_json::to_vec(&ui).unwrap();
        let back = crate::UiState::from_json(&bytes).unwrap();
        assert_eq!(back.optimo.command_counts.get("format.bold"), Some(&1));
        let junk = br#"{"optimo": {"commandRecent": 7, "commandCounts": {"a": "x"}, "inspector": false}}"#;
        let back = crate::UiState::from_json(junk).unwrap();
        assert!(back.optimo.command_recent.is_empty() && back.optimo.command_counts.is_empty());
        assert!(!back.optimo.inspector);
        let s = String::from_utf8(serde_json::to_vec(&back).unwrap()).unwrap();
        assert!(s.contains("\"optimo\":") && !s.contains("\"margin\":"), "{s}");
    }

    /// Settings saved while the app was called Margin (`"margin": {…}` in `ui.json`) come back
    /// under the new key; when both are there, the new one wins.
    #[test]
    fn margins_layout_settings_survive_the_rename() {
        let old = br#"{"margin": {"classicRibbon": true, "inspector": false, "textWidth": "narrow", "commandRecent": ["view.ruler"], "commandCounts": {"view.ruler": 3}}}"#;
        let back = crate::UiState::from_json(old).unwrap();
        assert!(back.optimo.classic_ribbon && !back.optimo.inspector);
        assert_eq!(back.optimo.text_width, crate::mac_ui::TextWidth::Narrow);
        assert_eq!(back.optimo.command_recent, ["view.ruler"]);
        assert_eq!(back.optimo.command_counts.get("view.ruler"), Some(&3));
        let both = br#"{"margin": {"classicRibbon": true}, "optimo": {"classicRibbon": false}}"#;
        assert!(!crate::UiState::from_json(both).unwrap().optimo.classic_ribbon);
    }
}
