//! Phase 6 (HANDOFF §6): the command field. A 300 × 28 search field in the toolbar centre with a
//! 480-wide results popover that searches the same registry as the old Search Commands dialog.
//!
//! Everything the field shows is a row ([`Row`]), built by pure functions ([`search`], [`rows`]) so
//! the ranking and the hidden assistant paths are tested without a window. The assistant rows
//! (Ask Claude, quick asks, the Commands / Ask toggle) exist but only show while
//! [`crate::review_panel::assistant_enabled`] is true, which is false until Settings › Assistant.

use std::collections::BTreeMap;

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::{CommandSpec, Registry};

use crate::WordApp;
use crate::mac_controls as mc;
use crate::skin::OptimoTokens;
use crate::theme::Tokens;

pub const FIELD_W: f32 = 300.0;
pub const FIELD_H: f32 = 28.0;
pub const POPOVER_W: f32 = 480.0;
pub const ROW_H: f32 = 28.0;
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

/// The ask the quick-ask rows send; the label is what the row shows.
pub const QUICK_ASKS: [(&str, &str); 4] = [
    ("Shorten", "Shorten this."),
    ("Fix grammar and spelling", "Fix grammar and spelling."),
    ("Make it plainer", "Make this plainer."),
    ("Check facts against a file…", "Check the facts in this against a file."),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Commands,
    Ask,
}

/// What an ask is about: the selection captured when the field opened (with its word count),
/// else the section where the caret is ("This section"). Never the whole document by default.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Scope {
    pub words: usize,
    /// The person's selection at the moment the field opened. The ask uses it even when focus
    /// moving to the field (or a key that reached the page) changed the live selection since.
    pub sel: Option<wordcraft_engine::Selection>,
}

impl Scope {
    /// The scope of `app`'s current selection, if it has text.
    pub fn capture(app: &WordApp) -> Option<Scope> {
        let text = app.session.selected_text();
        (!text.trim().is_empty()).then(|| Scope { words: text.split_whitespace().count(), sel: Some(app.session.sel.clone()) })
    }
}

/// The field's transient state (the counts live in [`crate::mac_ui::OptimoPrefs`]).
#[derive(Default)]
pub struct CommandField {
    pub query: String,
    pub open: bool,
    /// Take keyboard focus (and select the text) on the next frame.
    pub focus: bool,
    pub mode: Mode,
    /// Index of the highlighted row.
    pub sel: usize,
    /// The selection the ask is scoped to, captured when the field opened.
    pub scope: Option<Scope>,
    /// "This section “…”", captured when the field opened without a selection.
    pub section: String,
    /// Set by the toolbar each frame: whether the field fits (a narrow window drops it).
    pub field_visible: bool,
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
    /// "Ask Claude “…”" with the typed text.
    Ask(String),
    /// A quick ask: label, instruction.
    Quick(&'static str, &'static str),
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
            if tokens.iter().all(|t| hay.contains(t.as_str())) {
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

/// The popover's rows. Without the assistant: the matching commands. With it: the "Ask Claude"
/// row last, or first when nothing matches or the query reads as a sentence; in Ask mode the
/// quick asks and the ask row only; `files`: a folder is allowed, so "Check facts against a file…" shows.
pub fn rows(hits: &[&CommandSpec], query: &str, mode: Mode, assistant: bool, files: bool) -> Vec<Row> {
    let command_rows = hits.iter().map(|c| Row::Command { id: c.id, label: c.label, location: c.location });
    if !assistant {
        return command_rows.collect();
    }
    let q = query.trim();
    let ask = (!q.is_empty()).then(|| Row::Ask(q.to_string()));
    match mode {
        // What the person typed comes first: ↩ sends it, not a quick ask.
        Mode::Ask => ask.into_iter().chain(QUICK_ASKS.iter().filter(|(l, _)| files || *l != NEEDS_FILES).map(|(l, i)| Row::Quick(l, i))).collect(),
        Mode::Commands => {
            let first = ask.is_some() && (hits.is_empty() || sentence_like(q));
            let mut out: Vec<Row> = Vec::new();
            if first {
                out.extend(ask.clone());
            }
            out.extend(command_rows);
            if !first {
                out.extend(ask);
            }
            out
        }
    }
}

/// Quick asks that need folder access (Settings › Assistant S5) stay hidden until a folder is allowed.
pub const NEEDS_FILES: &str = "Check facts against a file…";

/// Send an ask to the assistant: the Assistant panel opens and shows it working. Scoped to the
/// selection when the field was opened on one, else the whole document.
pub fn submit_ask(app: &mut WordApp, instruction: &str, scope: &Scope) -> Result<(), String> {
    let kind = scope_kind(app, scope);
    crate::assistant_panel::ask(app, instruction, kind).map(|_| ())
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

/// Focus the field (Help › Search Commands…, the shortcut, Alt+Q). In the Classic ribbon layout
/// the old Search Commands dialog still serves, so it opens there instead.
pub fn open(app: &mut WordApp, query: &str) {
    if !crate::mac_ui::active(app) || !app.command_field.field_visible {
        app.dialog = Some(crate::dialogs::Dialog::Commands { query: query.to_string() });
        return;
    }
    let f = &mut app.command_field;
    f.query = query.to_string();
    f.open = true;
    f.focus = true;
    f.mode = Mode::Commands;
    f.sel = 0;
    f.scroll_y = 0.0;
    capture_scope(app);
}

/// Take the selection (or the section's name) as the ask's scope, at the moment the field opens.
fn capture_scope(app: &mut WordApp) {
    app.command_field.scope = Scope::capture(app);
    app.command_field.section = if app.command_field.scope.is_none() { section_label(app) } else { String::new() };
}

/// The context menu's "Ask Claude…": the field, pre-scoped to the selection, in Ask mode.
pub fn open_ask(app: &mut WordApp) {
    open(app, "");
    app.command_field.mode = Mode::Ask;
}

fn close(app: &mut WordApp, ctx: &egui::Context) {
    let f = &mut app.command_field;
    f.open = false;
    f.focus = false;
    f.query.clear();
    f.mode = Mode::Commands;
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
        Row::Quick(label, _) if *label == NEEDS_FILES => {
            // Opens the list of allowed files; the ask is sent when one is picked.
            let scope = app.command_field.scope.clone().unwrap_or_default();
            let kind = scope_kind(app, &scope);
            if let Err(e) = crate::assistant_panel::open_file_picker(app, kind) {
                app.status(e);
            }
        }
        Row::Quick(_, instruction) => ask(app, instruction),
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

/// The field and its popover. `rect` is where the field sits in the toolbar.
pub fn show(app: &mut WordApp, ui: &mut Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let m = OptimoTokens::get(&ctx);
    let assistant = crate::review_panel::assistant_enabled();
    let edit_id = egui::Id::new("command_field_edit");

    // Keys first, so the text field doesn't also act on them.
    let focused = ctx.memory(|mem| mem.has_focus(edit_id));
    let mut pending: Option<Key> = None;
    // egui drops focus on Escape before we see it, so Escape is read without needing focus.
    if app.command_field.open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        pending = Some(Key::Escape);
    } else if app.command_field.open && focused {
        pending = take_keys(&ctx, assistant);
    }

    let f = &mut app.command_field;
    if f.query != f.last_query {
        f.last_query = f.query.clone();
        f.sel = 0;
        f.scroll_y = 0.0;
        f.scroll_to = true;
    }
    let reg = app.session.registry.clone();
    let usage = Usage { recent: &app.ui.optimo.command_recent, counts: &app.ui.optimo.command_counts };
    let hits = if app.command_field.mode == Mode::Ask { Vec::new() } else { search(&reg, &app.command_field.query, usage) };
    let files = crate::assistant_panel::facts_available(app);
    let list = rows(&hits, &app.command_field.query, app.command_field.mode, assistant, files);
    let f = &mut app.command_field;
    if f.sel >= list.len() {
        f.sel = list.len().saturating_sub(1);
    }

    let mut run: Option<usize> = None;
    match pending {
        Some(Key::Down) if !list.is_empty() => {
            f.sel = (f.sel + 1) % list.len();
            f.scroll_to = true;
        }
        Some(Key::Up) if !list.is_empty() => {
            f.sel = (f.sel + list.len() - 1) % list.len();
            f.scroll_to = true;
        }
        Some(Key::Enter) if !list.is_empty() => run = Some(f.sel),
        Some(Key::Tab) => {
            f.mode = if f.mode == Mode::Commands { Mode::Ask } else { Mode::Commands };
            f.sel = 0;
            f.scroll_y = 0.0;
            f.scroll_to = true;
        }
        Some(Key::Escape) => {
            close(app, &ctx);
            paint_field(app, ui, rect, &t, &m, false, edit_id);
            return;
        }
        _ => {}
    }

    let has_focus = paint_field(app, ui, rect, &t, &m, true, edit_id);
    if has_focus && !app.command_field.open {
        // Opened by a click into the field: the scope is taken now, like ⌘Y does.
        app.command_field.open = true;
        capture_scope(app);
    }
    let f = &mut app.command_field;

    // Popover.
    if f.open {
        let mut clicked: Option<usize> = None;
        let area = egui::Area::new(egui::Id::new("command_field_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2((rect.center().x - POPOVER_W / 2.0).max(8.0), rect.max.y + 6.0))
            .show(&ctx, |ui| {
                egui::Frame::new()
                    .fill(m.popover_bg)
                    .stroke(Stroke::new(1.0, m.popover_border))
                    .corner_radius(8)
                    .shadow(m.popover_shadow)
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(POPOVER_W - 12.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        clicked = list_ui(app, ui, &list, &t, assistant);
                    });
            });
        let popover_rect = area.response.rect;
        if let Some(i) = clicked {
            run = Some(i);
        }
        // A press outside both the field and the popover closes it.
        let outside =
            ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p) && !popover_rect.contains(p)));
        if outside && run.is_none() {
            let f = &mut app.command_field;
            f.open = false;
            f.query.clear();
            f.mode = Mode::Commands;
        }
    }
    if let Some(i) = run
        && let Some(row) = list.get(i)
    {
        run_row(app, &ctx, row);
    }
}

enum Key {
    Up,
    Down,
    Enter,
    Tab,
    Escape,
}

/// Take the field's navigation keys out of this frame's input.
fn take_keys(ctx: &egui::Context, assistant: bool) -> Option<Key> {
    ctx.input_mut(|i| {
        let none = egui::Modifiers::NONE;
        if i.consume_key(none, egui::Key::ArrowDown) {
            Some(Key::Down)
        } else if i.consume_key(none, egui::Key::ArrowUp) {
            Some(Key::Up)
        } else if i.consume_key(none, egui::Key::Enter) {
            Some(Key::Enter)
        } else if assistant && i.consume_key(none, egui::Key::Tab) {
            Some(Key::Tab)
        } else {
            None
        }
    })
}

/// The field itself; true when it has keyboard focus. `live` is false for the frame that closes it.
fn paint_field(app: &mut WordApp, ui: &mut Ui, rect: Rect, t: &Tokens, m: &OptimoTokens, live: bool, edit_id: egui::Id) -> bool {
    let assistant = crate::review_panel::assistant_enabled();
    let f = &mut app.command_field;
    let had_focus = ui.ctx().memory(|mem| mem.has_focus(edit_id));
    let focused = live && (had_focus || f.focus);
    let border = if focused { t.accent } else { t.input_border };
    ui.painter().rect(rect, mc::RADIUS, m.field_bg, Stroke::new(1.0, border), egui::StrokeKind::Inside);
    if focused {
        ui.painter().rect_stroke(rect.expand(1.5), mc::RADIUS + 1.5, Stroke::new(3.0, m.focus_ring), egui::StrokeKind::Outside);
    }
    crate::icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(rect.min.x + 15.0, rect.center().y), vec2(15.0, 15.0)),
        "search",
        t.text_dim,
        t.accent,
    );
    let placeholder = if assistant {
        crate::i18n::fmt(tl!("Search commands or ask {name}"), &[("name", &crate::review_panel::shown_assistant_name())])
    } else {
        tl!("Search commands").to_string()
    };
    let hint_w = if focused || !f.query.is_empty() { 0.0 } else { 44.0 };
    let edit_rect = Rect::from_min_max(pos2(rect.min.x + 30.0, rect.min.y + 2.0), pos2(rect.max.x - 8.0 - hint_w, rect.max.y - 2.0));
    let te = ui.put(
        edit_rect,
        egui::TextEdit::singleline(&mut f.query)
            .id(edit_id)
            .frame(egui::Frame::NONE)
            .vertical_align(egui::Align::Center)
            .font(mc::font())
            .text_color(t.text)
            .hint_text(egui::RichText::new(&placeholder).color(t.text_dim).font(mc::font()))
            // ⇥ switches Commands / Ask; it must not move the focus to the page (typing would
            // then go into the document, over the selection).
            .lock_focus(true)
            .desired_width(edit_rect.width()),
    );
    if f.focus && live {
        te.request_focus();
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), edit_id) {
            let end = egui::text::CCursor::new(f.query.chars().count());
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ui.ctx(), edit_id);
        }
        f.focus = false;
    }
    if te.changed() {
        f.open = true;
    }
    if hint_w > 0.0 {
        ui.painter().text(pos2(rect.max.x - 10.0, rect.center().y), Align2::RIGHT_CENTER, SHORTCUT_LABEL, mc::font(), t.text_dim);
    }
    te.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &placeholder));
    live && te.has_focus()
}

/// The rows, a hairline and the footer hints. Returns the row clicked.
fn list_ui(app: &mut WordApp, ui: &mut Ui, list: &[Row], t: &Tokens, assistant: bool) -> Option<usize> {
    let mut clicked = None;
    // The scope line: what an ask is about.
    if assistant {
        let scope = match &app.command_field.scope {
            Some(s) => format!("{} · {}", tl!("Selection"), crate::i18n::fmt(tl!("{n} words"), &[("n", &s.words.to_string())])),
            None => app.command_field.section.clone(),
        };
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
        ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, scope, mc::font(), t.text_dim);
    }
    if list.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        let text = if app.command_field.query.trim().is_empty() {
            tl!("Type to search every command").to_string()
        } else {
            crate::i18n::fmt(tl!("No commands match “{query}”"), &[("query", app.command_field.query.trim())])
        };
        ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, text, mc::font(), t.text_dim);
    } else {
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
    }
    // Footer hints.
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.min.y + 2.0, Stroke::new(1.0, t.border));
    let mut hints = format!("↩ {}     ↑↓ {}     esc {}", tl!("Run"), tl!("Move"), tl!("Close"));
    if assistant {
        let other = if app.command_field.mode == Mode::Commands { tl!("Ask") } else { tl!("Commands") };
        hints.push_str(&format!("     ⇥ {other}"));
    }
    ui.painter().text(pos2(r.min.x + 10.0, r.center().y + 1.0), Align2::LEFT_CENTER, hints, mc::font(), t.text_dim);
    clicked
}

/// One row: 28 high. True when clicked.
fn paint_row(app: &WordApp, ui: &mut Ui, row: &Row, highlighted: bool, t: &Tokens) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if highlighted || resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.checked);
    }
    let font = mc::font();
    match row {
        Row::Command { id, label, location } => {
            let enabled = app.session.registry.get(id).is_none_or(|s| (s.enabled)(&app.session).is_none());
            let ink = if enabled { t.text } else { t.text_dim };
            let sc = crate::widgets::shortcut_text(app, id);
            let sc_w = if sc.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(sc.clone(), font.clone(), t.text_dim).size().x + 12.0 };
            if !sc.is_empty() {
                ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, &sc, font.clone(), t.text_dim);
            }
            let name = ui.painter().layout_no_wrap(tl!(label).to_string(), font.clone(), ink);
            let name_w = name.size().x;
            let avail = r.width() - 20.0 - sc_w;
            ui.painter().galley(pos2(r.min.x + 10.0, r.center().y - name.size().y / 2.0), name, ink);
            let path_w = avail - name_w - 10.0;
            if path_w > 40.0 {
                let mut job = egui::text::LayoutJob::simple_singleline(crate::i18n::location(location), font, t.text_dim);
                job.wrap = egui::text::TextWrapping::truncate_at_width(path_w);
                let g = ui.painter().layout_job(job);
                ui.painter().galley(pos2(r.min.x + 10.0 + name_w + 10.0, r.center().y - g.size().y / 2.0), g, t.text_dim);
            }
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tl!(label)));
        }
        Row::Ask(text) => {
            let label = crate::i18n::fmt(tl!("Ask {name} “{text}”"), &[("name", &crate::review_panel::shown_assistant_name()), ("text", text)]);
            ask_row(ui, r, &label, t, &font);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        }
        Row::Quick(label, _) => {
            ask_row(ui, r, tl!(label), t, &font);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!(label)));
        }
    }
    resp.clicked()
}

fn ask_row(ui: &Ui, r: Rect, label: &str, t: &Tokens, font: &egui::FontId) {
    let assistant = OptimoTokens::get(ui.ctx()).assistant;
    crate::icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(r.min.x + 19.0, r.center().y), vec2(15.0, 15.0)),
        "assistant",
        assistant,
        assistant,
    );
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), font.clone(), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(r.width() - 40.0);
    let g = ui.painter().layout_job(job);
    ui.painter().galley(pos2(r.min.x + 36.0, r.center().y - g.size().y / 2.0), g, Color32::PLACEHOLDER);
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The assistant rows: absent while it is off; present, ordered, and toggled when on.
    #[test]
    fn ask_rows_follow_the_assistant_switch() {
        let s = session();
        let bold = search(&s.registry, "bold", NO_USE);
        assert!(!crate::review_panel::assistant_enabled(), "off until Settings › Assistant");
        let off = rows(&bold, "bold", Mode::Commands, false, false);
        assert!(off.iter().all(|r| matches!(r, Row::Command { .. })), "no ask row while off");
        assert!(rows(&bold, "bold", Mode::Ask, false, false).iter().all(|r| matches!(r, Row::Command { .. })), "no Ask mode while off");

        let on = rows(&bold, "bold", Mode::Commands, true, false);
        assert!(matches!(on.last(), Some(Row::Ask(t)) if t == "bold"), "ask row is last");
        let sentence = rows(&bold, "make the second paragraph shorter", Mode::Commands, true, false);
        assert!(matches!(sentence.first(), Some(Row::Ask(_))), "a sentence puts the ask row first");
        let none = rows(&[], "zzz", Mode::Commands, true, false);
        assert_eq!(none, [Row::Ask("zzz".into())], "nothing matches: only the ask row");
        let ask_mode = rows(&bold, "tighten it", Mode::Ask, true, false);
        assert!(matches!(ask_mode.first(), Some(Row::Ask(_))), "what was typed comes first: ↩ sends it");
        assert!(matches!(ask_mode.get(1), Some(Row::Quick("Shorten", _))));
        assert_eq!(ask_mode.iter().filter(|r| matches!(r, Row::Command { .. })).count(), 0);
    }

    #[test]
    fn asking_without_an_assistant_is_an_error_not_a_panic() {
        let mut app = WordApp::new(session(), crate::Services::default());
        let r = submit_ask(&mut app, "Shorten this.", &Scope { words: 3, ..Default::default() });
        assert!(r.as_ref().is_err_and(|e| e.contains("Let an assistant work in Optimo")), "{r:?}");
        ask(&mut app, "Shorten this.");
        assert!(app.status_msg.as_ref().is_some_and(|(m, _)| m.contains("Let an assistant work in Optimo")));
        assert!(!app.assistant_panel.working());
    }

    #[test]
    fn the_file_quick_ask_waits_for_folder_access() {
        let on = rows(&[], "", Mode::Ask, true, false);
        assert!(on.iter().all(|r| !matches!(r, Row::Quick(l, _) if *l == NEEDS_FILES)), "{on:?}");
        assert_eq!(on.len(), 3, "Shorten, Fix grammar and spelling, Make it plainer");
        // With a folder allowed it shows, last of the quick asks (after the typed ask).
        let with = rows(&[], "tighten it", Mode::Ask, true, true);
        let labels: Vec<&str> = with.iter().filter_map(|r| if let Row::Quick(l, _) = r { Some(*l) } else { None }).collect();
        assert_eq!(labels, ["Shorten", "Fix grammar and spelling", "Make it plainer", NEEDS_FILES]);
        assert!(matches!(with.first(), Some(Row::Ask(_))));
        // Never while the assistant itself is off.
        assert!(rows(&[], "", Mode::Ask, false, true).is_empty());
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

    /// The Classic ribbon keeps the old Search Commands dialog; the Optimo layout never opens it.
    #[test]
    fn classic_layout_still_opens_the_dialog() {
        let mut h = harness();
        h.state_mut().ui.optimo.classic_ribbon = true;
        h.state_mut().run("ui.dialog", json!({"name": "commands"})).unwrap();
        assert!(matches!(h.state().dialog, Some(crate::dialogs::Dialog::Commands { .. })));
        h.state_mut().dialog = None;
        h.state_mut().ui.optimo.classic_ribbon = false;
        h.state_mut().run("ui.dialog", json!({"name": "commands"})).unwrap();
        assert!(h.state().dialog.is_none() && h.state().command_field.open);
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
