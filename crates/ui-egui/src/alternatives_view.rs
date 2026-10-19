//! O5 (OPTIMO.md §6, A1–A3): Alternatives, the popover card inside the page.
//!
//! * ⌥⌘A or ⌘Y › Alternatives with a selection (without one, the sentence at the caret is
//!   selected first) asks the co-writer for three other ways to say it (`cowriter::start`; the
//!   engine's `cowriter/alternatives.rs` writes the prompt and reads the answer). Nothing lands
//!   until the person picks one.
//! * The card sits under the selected sentence, page width, radius 10. Header: the dotted mark,
//!   "Other ways to say it · your words kept where they work", and a Tone popup. Rows 34 high:
//!   a 64 wide label (Yours · Sharper · Plainer · Warmer), then the sentence; only the changed
//!   words are in the assistant colour with a dotted underline (`assistant_edits::word_diff`).
//!   Footer: "1–3 use one · ⇥ more like one · ✎ edit before using", "Arrives as a suggestion".
//! * Keys: 1–3 use one, ↑ ↓ move, ⏎ uses the highlighted one, ⇥ asks for more like it, E edits it
//!   first, Esc keeps theirs (rule 2). Typing, or moving the caret, closes the card.
//! * Using one is `cowriter::apply`: one tracked replacement by the AI author, worded word by word
//!   (rule 3), through the same guards as every other co-writer output. Undo takes it back.

use egui::{Align, FontId, Layout, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_assistant::{AssistantError, ChatReply};
use wordcraft_engine::assistant_edits::{self as edits, ScopeKind, ScopeText};
use wordcraft_engine::cowriter::{self, Call, Output, Reply, Verb};
use wordcraft_engine::doc::Pos;
use wordcraft_layout::DocLayout;

use crate::WordApp;
use crate::cowriter::Status;
use crate::mac_controls as mc;
use crate::skin::OptimoTokens;
use crate::theme::Tokens;

/// The Tone popup's choices: the label, and what the engine is told (none = like the document).
const TONES: [(&str, Option<&str>); 5] = [
    ("Like the rest of the doc", None),
    ("Plainer", Some("plainer")),
    ("Warmer", Some("warmer")),
    ("More formal", Some("more formal")),
    ("Shorter", Some("shorter")),
];
const ROW_H: f32 = 34.0;
const LABEL_W: f32 = 64.0;
const RADIUS: f32 = 10.0;
const PAD: f32 = 12.0;

enum State {
    Loading,
    Ready,
    Failed(String),
}

/// The open card.
pub struct Card {
    doc: u64,
    /// The run it belongs to (for the margin's log).
    run: Option<usize>,
    scope: ScopeText,
    /// Where the selection was when it opened: the card closes when it moves.
    sel: (Pos, Pos),
    /// The person's own sentence(s).
    original: String,
    alts: Vec<String>,
    state: State,
    /// Asked for "more like" this one: the rows read "Like it".
    more: bool,
    /// The highlighted row: 0 is Yours.
    hi: usize,
    tone: usize,
    /// ✎: the text being edited before it lands.
    edit: Option<String>,
    /// Why the last pick did not land.
    problem: Option<String>,
    reveal: bool,
}

impl Card {
    pub fn editing(&self) -> bool {
        self.edit.is_some()
    }
}

enum Act {
    Close,
    Pick(usize),
    More(usize),
    Edit(usize),
    Tone(usize),
    Commit(String),
    CancelEdit,
    Move(usize),
}

/// A run starts: the card opens at once, waiting for the answer.
pub fn begin(app: &mut WordApp, scope: &ScopeText, options: &serde_json::Value) {
    let tone = options.get("tone").and_then(|t| t.as_str()).and_then(|t| TONES.iter().position(|(_, id)| *id == Some(t))).unwrap_or(0);
    app.cowriter.alts = Some(Card {
        doc: app.session.document_id(),
        run: None,
        scope: scope.clone(),
        sel: app.session.sel.ordered(),
        original: scope.text.trim().to_string(),
        alts: Vec::new(),
        state: State::Loading,
        more: options.get("moreLike").is_some_and(|m| m.as_str().is_some_and(|s| !s.trim().is_empty())),
        hi: 0,
        tone,
        edit: None,
        problem: None,
        reveal: true,
    });
}

/// The answer arrived (or the request failed): read it, guarded, into the card's rows.
pub fn arrived(
    app: &mut WordApp,
    doc: u64,
    run: usize,
    scope: &ScopeText,
    options: &serde_json::Value,
    name: &str,
    r: Result<ChatReply, AssistantError>,
) {
    let (status, report, mut lines, rows, state) = match r {
        Ok(reply) => {
            let call = Call { session: &app.session, scope, options, name };
            match cowriter::run(Verb::Alternatives, &call, Some(Reply { text: &reply.text, stop_reason: reply.stop_reason.as_deref() })) {
                Ok(ran) => {
                    let original = scope.text.trim();
                    let rows: Vec<String> = ran
                        .outputs
                        .into_iter()
                        .filter_map(|o| match o {
                            Output::Replacement { anchor, with } if anchor == original => Some(with),
                            _ => None,
                        })
                        .collect();
                    let state = if rows.is_empty() {
                        let why =
                            if ran.report.trim().is_empty() { tl!("No other ways found. Yours reads well.").to_string() } else { ran.report.clone() };
                        State::Failed(why)
                    } else {
                        State::Ready
                    };
                    (Status::Done, ran.report, ran.lines, rows, state)
                }
                Err(e) => (Status::Failed, String::new(), vec![e.clone()], Vec::new(), State::Failed(e)),
            }
        }
        Err(AssistantError::Stopped) => {
            app.cowriter.alts = None;
            return;
        }
        Err(e) => {
            // Never holds the key (wordcraft_assistant::AssistantError).
            log::warn!("co-writer: {e}");
            let why = e.to_string();
            let why: String = why.chars().next().map(|c| c.to_uppercase().chain(why.chars().skip(1)).collect()).unwrap_or_default();
            (Status::Failed, String::new(), vec![why.clone()], Vec::new(), State::Failed(why))
        }
    };
    if let Some(run) = crate::cowriter::run_mut(app, doc, run) {
        run.status = status;
        run.report = report;
        run.lines.append(&mut lines);
    }
    if let Some(card) = app.cowriter.alts.as_mut().filter(|c| c.doc == doc) {
        card.run = Some(run);
        card.alts = rows;
        card.state = state;
        card.reveal = true;
    }
}

/// Keys while the card is open. `true` when one was used (the canvas then ignores the frame's
/// events, so a 1 is not typed into the document).
pub fn keys(app: &mut WordApp, ctx: &egui::Context) -> bool {
    let Some(card) = app.cowriter.alts.as_ref() else { return false };
    if card.editing() {
        return false;
    }
    let n = card.alts.len();
    let hi = card.hi;
    let mut act = None;
    for e in ctx.input(|i| i.events.clone()) {
        let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
        if modifiers.command || modifiers.alt || modifiers.ctrl {
            continue;
        }
        use egui::Key as K;
        act = match key {
            K::Escape => Some(Act::Close),
            K::Num1 | K::Num2 | K::Num3 if matches!(card.state, State::Ready) => {
                let i = key as usize - K::Num1 as usize + 1;
                (i <= n).then_some(Act::Pick(i))
            }
            K::ArrowDown => Some(Act::Move((hi + 1).min(n))),
            K::ArrowUp => Some(Act::Move(hi.saturating_sub(1))),
            K::Enter if matches!(card.state, State::Ready) => Some(Act::Pick(hi)),
            K::Tab if matches!(card.state, State::Ready) => Some(Act::More(hi)),
            K::E if hi > 0 => Some(Act::Edit(hi)),
            _ => None,
        };
        if act.is_some() {
            break;
        }
    }
    let used = act.is_some();
    if let Some(a) = act {
        run(app, a);
    }
    used
}

fn run(app: &mut WordApp, act: Act) {
    match act {
        Act::Close => app.cowriter.alts = None,
        Act::Move(i) => {
            if let Some(c) = app.cowriter.alts.as_mut() {
                c.hi = i;
            }
        }
        Act::Pick(0) => app.cowriter.alts = None,
        Act::Pick(i) => {
            let text = app.cowriter.alts.as_ref().and_then(|c| c.alts.get(i - 1).cloned());
            if let Some(t) = text {
                land(app, t);
            }
        }
        Act::Commit(t) => land(app, t),
        Act::Edit(i) => {
            if let Some(c) = app.cowriter.alts.as_mut() {
                c.edit = c.alts.get(i - 1).cloned();
            }
        }
        Act::CancelEdit => {
            if let Some(c) = app.cowriter.alts.as_mut() {
                c.edit = None;
            }
        }
        Act::Tone(i) => ask_again(app, i, None),
        Act::More(i) => {
            let like = app.cowriter.alts.as_ref().and_then(|c| if i == 0 { None } else { c.alts.get(i - 1).cloned() });
            let tone = app.cowriter.alts.as_ref().map_or(0, |c| c.tone);
            ask_again(app, tone, like);
        }
    }
}

fn ask_again(app: &mut WordApp, tone: usize, like: Option<String>) {
    let mut options = json!({});
    if let Some(id) = TONES.get(tone).and_then(|(_, id)| *id) {
        options["tone"] = json!(id);
    }
    if let Some(l) = like {
        options["moreLike"] = json!(l);
    }
    if let Err(e) = crate::cowriter::start(app, Verb::Alternatives, ScopeKind::Selection, options) {
        app.status(e);
    }
}

/// Use `text` in place of the person's sentence: one tracked replacement by the AI author.
fn land(app: &mut WordApp, text: String) {
    let Some(card) = app.cowriter.alts.as_ref() else { return };
    let (doc, run, scope, original) = (card.doc, card.run, card.scope.clone(), card.original.clone());
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text == original {
        app.cowriter.alts = None;
        return;
    }
    crate::assistant_link::sync_policy(app);
    let name = app.ui.assistant.name();
    let o = Output::Replacement { anchor: original, with: text };
    let landed = cowriter::apply(&mut app.session, &name, Verb::Alternatives, &scope, vec![o]);
    if landed.applied.is_empty() {
        let why = landed.lines.first().cloned().unwrap_or_else(|| tl!("It could not be used.").to_string());
        if let Some(c) = app.cowriter.alts.as_mut() {
            c.problem = Some(why);
            c.edit = None;
        }
        return;
    }
    app.ui.assistant.remember_name(&name);
    if let Some(r) = run.and_then(|i| crate::cowriter::run_mut(app, doc, i)) {
        r.applied.extend(landed.applied);
    }
    app.cowriter.alts = None;
}

/// The new text of `alt` as spans: (text, changed).
fn spans(original: &str, alt: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut at = 0;
    for r in edits::word_diff(original, alt) {
        if let Some(kept) = original.get(at..r.start).filter(|k| !k.is_empty()) {
            out.push((kept.to_string(), false));
        }
        if !r.with.is_empty() {
            out.push((r.with.clone(), true));
        }
        at = r.end;
    }
    if let Some(rest) = original.get(at..).filter(|k| !k.is_empty()) {
        out.push((rest.to_string(), false));
    }
    if out.is_empty() {
        out.push((alt.to_string(), false));
    }
    out
}

/// Draw the card under the selected sentence.
pub fn paint(app: &mut WordApp, ui: &mut Ui, rects: &[Rect], layout: &DocLayout, scale: f32) {
    let Some(card) = app.cowriter.alts.as_ref() else { return };
    if card.doc != app.session.document_id() || card.sel != app.session.sel.ordered() {
        app.cowriter.alts = None;
        return;
    }
    let Some(c) = layout.caret_on(&card.sel.1, app.session.page_hint) else { return };
    let (Some(pr), Some(page)) = (rects.get(c.page), layout.pages.get(c.page)) else { return };
    let ps = crate::canvas::page_screen_scale(*pr, page, scale);
    let width = (pr.width() - 48.0).max(320.0);
    let at = pos2(pr.min.x + 24.0, pr.min.y + (c.top + c.height) * ps + 12.0);
    let t = Tokens::get(ui.ctx());
    let m = OptimoTokens::get(ui.ctx());
    let size = (PAGE_FONT * ps).clamp(13.0, 20.0);
    let mut acts: Vec<Act> = Vec::new();
    let Some(card) = app.cowriter.alts.as_mut() else { return };
    let mut area_rect = Rect::NOTHING;
    egui::Area::new(egui::Id::new("optimo_alternatives")).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
        let frame = egui::Frame::new()
            .fill(m.popover_bg)
            .stroke(Stroke::new(1.0, m.popover_border))
            .corner_radius(RADIUS)
            .shadow(m.popover_shadow)
            .inner_margin(egui::Margin::same(PAD as i8));
        let shown = frame.show(ui, |ui| {
            ui.set_width(width - 2.0 * PAD);
            header(ui, card, &t, &m, &mut acts);
            ui.add_space(6.0);
            rows(ui, card, &t, &m, size, &mut acts);
            ui.add_space(8.0);
            footer(ui, card, &t, &mut acts);
        });
        area_rect = shown.response.rect;
    });
    if card.reveal && area_rect.is_positive() {
        card.reveal = false;
        ui.scroll_to_rect(area_rect.expand(24.0), Some(Align::Center));
    }
    // Esc inside the edit field keeps the card; a click outside the card and the sentence is the
    // canvas moving the caret, which closes it (above).
    for a in acts {
        run(app, a);
    }
}

/// The body size of a typical page, in points; the card scales with the page, within 13–20.
const PAGE_FONT: f32 = 11.0;

fn header(ui: &mut Ui, card: &Card, t: &Tokens, m: &OptimoTokens, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        for i in 0..3 {
            ui.painter().circle_filled(pos2(r.min.x + 2.0 + 5.0 * i as f32, r.center().y), 1.4, m.assistant);
        }
        ui.label(egui::RichText::new(tl!("Other ways to say it · your words kept where they work")).font(mc::font()).color(t.text_dim));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let items: Vec<String> = TONES.iter().map(|(l, _)| tl!(l).to_string()).collect();
            let current = items.get(card.tone).cloned().unwrap_or_default();
            if let Some(pick) = mc::popup_button(ui, "alternatives_tone", 190.0, &current, &items, false)
                && let Some(i) = items.iter().position(|x| *x == pick)
                && i != card.tone
            {
                acts.push(Act::Tone(i));
            }
        });
    });
}

fn rows(ui: &mut Ui, card: &mut Card, t: &Tokens, m: &OptimoTokens, size: f32, acts: &mut Vec<Act>) {
    let font = FontId::proportional(size);
    let labels: [&str; 3] =
        if card.more { [tl!("Like it"), tl!("Like it"), tl!("Like it")] } else { [tl!("Sharper"), tl!("Plainer"), tl!("Warmer")] };
    // Yours.
    let yours = vec![(card.original.clone(), false)];
    if row(ui, tl!("Yours"), &yours, "esc", card.hi == 0, &font, t, m).clicked() {
        acts.push(Act::Close);
    }
    match &card.state {
        State::Loading => {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tl!("Looking for other ways to say it…")).font(mc::font()).color(t.text_dim));
            ui.ctx().request_repaint();
        }
        State::Failed(why) => {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(why.as_str()).font(mc::font()).color(t.text_dim));
        }
        State::Ready => {
            for (i, alt) in card.alts.iter().enumerate() {
                let hint = (i + 1).to_string();
                let label = labels.get(i).copied().unwrap_or("");
                let r = row(ui, label, &spans(&card.original, alt), &hint, card.hi == i + 1, &font, t, m);
                if card.edit.is_some() && card.hi == i + 1 {
                    continue;
                }
                if r.clicked() {
                    acts.push(Act::Pick(i + 1));
                } else if r.hovered() && card.hi != i + 1 && ui.input(|i| i.pointer.delta().length() > 0.0) {
                    acts.push(Act::Move(i + 1));
                }
            }
        }
    }
    if let Some(text) = card.edit.as_mut() {
        ui.add_space(6.0);
        let te = ui.add(egui::TextEdit::multiline(text).font(font.clone()).desired_rows(1).desired_width(f32::INFINITY));
        te.request_focus();
        let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter) && !i.modifiers.shift, i.key_pressed(egui::Key::Escape)));
        if esc {
            acts.push(Act::CancelEdit);
        } else if enter {
            acts.push(Act::Commit(text.clone()));
        }
        ui.label(egui::RichText::new(tl!("⏎ use it · esc back")).font(mc::font()).color(t.text_dim));
    }
    if let Some(p) = &card.problem {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(p.as_str()).font(mc::font()).color(m.assistant));
    }
}

/// One row: label, the sentence with its changed words marked, a key hint.
#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut Ui,
    label: &str,
    parts: &[(String, bool)],
    hint: &str,
    selected: bool,
    font: &FontId,
    t: &Tokens,
    m: &OptimoTokens,
) -> egui::Response {
    let w = ui.available_width();
    let text_w = (w - LABEL_W - 44.0).max(80.0);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = text_w;
    for (s, changed) in parts {
        job.append(s, 0.0, egui::TextFormat::simple(font.clone(), if *changed { m.assistant } else { t.text }));
    }
    let galley = ui.painter().layout_job(job);
    let h = (galley.size().y + 14.0).max(ROW_H);
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    if selected {
        ui.painter().rect_filled(r, 6.0, t.checked);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    let top = r.min.y + (h - galley.size().y) / 2.0;
    ui.painter().text(pos2(r.min.x + 10.0, r.min.y + h / 2.0), egui::Align2::LEFT_CENTER, label, mc::font(), t.text_dim);
    let origin = pos2(r.min.x + 10.0 + LABEL_W, top);
    // Dotted underline under the changed words, glyph by glyph.
    let changed: Vec<bool> = parts.iter().flat_map(|(s, c)| s.chars().map(move |_| *c)).collect();
    let mut k = 0usize;
    for prow in &galley.rows {
        let mut run: Option<(f32, f32)> = None;
        let y = origin.y + prow.pos.y + prow.row.height() - 1.0;
        for g in &prow.row.glyphs {
            let on = changed.get(k).copied().unwrap_or(false);
            k += 1;
            let (x0, x1) = (origin.x + prow.pos.x + g.pos.x, origin.x + prow.pos.x + g.pos.x + g.advance_width);
            match (&mut run, on) {
                (Some((_, end)), true) => *end = x1,
                (Some((a, b)), false) => {
                    crate::review_panel::mark_line(ui.painter(), pos2(*a, y), pos2(*b, y), m.assistant, true);
                    run = None;
                }
                (None, true) => run = Some((x0, x1)),
                (None, false) => {}
            }
        }
        if let Some((a, b)) = run {
            crate::review_panel::mark_line(ui.painter(), pos2(a, y), pos2(b, y), m.assistant, true);
        }
    }
    ui.painter().galley(origin, galley, t.text);
    ui.painter().text(pos2(r.max.x - 10.0, r.min.y + h / 2.0), egui::Align2::RIGHT_CENTER, hint, mc::font(), t.text_dim);
    resp
}

fn footer(ui: &mut Ui, card: &Card, t: &Tokens, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        let dim = |s: &str| egui::RichText::new(s.to_string()).font(mc::font()).color(t.text_dim);
        ui.label(dim(tl!("1–3 use one")));
        ui.label(dim("·"));
        let more = ui.add(egui::Label::new(dim(tl!("⇥ more like one"))).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand);
        if more.clicked() && matches!(card.state, State::Ready) {
            acts.push(Act::More(card.hi));
        }
        ui.label(dim("·"));
        let edit = ui.add(egui::Label::new(dim(tl!("E edit before using"))).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand);
        if edit.clicked() && card.hi > 0 {
            acts.push(Act::Edit(card.hi));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(dim(tl!("Arrives as a suggestion")));
        });
    });
}

/// `ui.assistant.alternatives`: read the open card, use a row (or edited text), or close it.
pub fn control(app: &mut WordApp, p: &serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(card) = app.cowriter.alts.as_ref() else { return Err("no Alternatives card is open".into()) };
    if p.get("close").and_then(|v| v.as_bool()) == Some(true) {
        app.cowriter.alts = None;
        return Ok(json!({"closed": true}));
    }
    if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
        run(app, Act::Commit(t.to_string()));
    } else if let Some(n) = p.get("pick").and_then(|v| v.as_u64()) {
        if n == 0 || n as usize > card.alts.len() {
            return Err("`pick` must be the number of a row, 1–3".into());
        }
        run(app, Act::Pick(n as usize));
    }
    Ok(match app.cowriter.alts.as_ref() {
        Some(c) => {
            json!({"open": true, "original": c.original, "alternatives": c.alts, "loading": matches!(c.state, State::Loading), "problem": c.problem})
        }
        None => json!({"open": false}),
    })
}
