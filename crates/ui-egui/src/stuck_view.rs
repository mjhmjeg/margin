//! O9 (OPTIMO.md §10, U1–U3): what I'm stuck shows while the caret rests in its empty paragraph.
//!
//! * **On the page:** a grey italic placeholder at the caret, painted over the page and never
//!   document text. It goes with the first keystroke (the paragraph is no longer empty) and with
//!   the caret leaving.
//! * **U1** Right margin, "Questions only you can answer": the model's 3–5 questions under a
//!   dotted rule. A click pins one as a comment by the AI author at the caret
//!   ([`wordcraft_engine::stuck::pin`]); it then stays in the margin as an ordinary note.
//! * **U2** "From your notes": earlier comments of this document and, with folders allowed in
//!   Settings › Co-writer, lines of linked files, each quoted with its origin. Computed here,
//!   never sent to the model.
//! * **U3** In `text_dim`: "Still stuck? Show a rough first line to argue with". The line came
//!   with the questions and was held back; the click lands it as one tracked insert by the AI
//!   author, the one text the co-writer adds in this phase.
//!
//! [`enter`] is the entry point for other phases (Structure's empty sections, O7): it takes the
//! paragraph (or none: the caret's section) and starts the run.

use egui::{Color32, Pos2, Rect, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::assistant_edits::{ScopeKind, ScopeText};
use wordcraft_engine::cowriter::{self, Output, Ran, Verb};
use wordcraft_engine::doc::{Path, Pos};
use wordcraft_engine::stuck::{self, Snippet};
use wordcraft_layout::DocLayout;

use crate::WordApp;
use crate::mac_controls as mc;
use crate::theme::Tokens;

/// One question and, once clicked, the comment it became.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub text: String,
    pub pinned: Option<u32>,
}

/// The view of the empty paragraph the person is stuck in.
pub struct StuckView {
    pub doc: u64,
    /// The empty paragraph (offset 0).
    pub at: Pos,
    pub name: String,
    scope: ScopeText,
    pub questions: Vec<Question>,
    /// The rough first line, held back until the person asks for it.
    first_line: Option<Output>,
    pub notes: Vec<Snippet>,
}

enum Act {
    Pin(usize),
    FirstLine,
}

/// Start I'm stuck for the caret's section, or after paragraph `at` (O7: an empty section).
/// Needs the co-writer to be on and a provider chosen, like every verb with a model.
pub fn enter(app: &mut WordApp, at: Option<Path>) -> Result<Value, String> {
    app.cowriter.stuck_at = at;
    let r = crate::cowriter::start(app, Verb::Stuck, ScopeKind::Section, json!({}));
    app.cowriter.stuck_at = None;
    r
}

/// Called by `cowriter::start` once the run is certain: put the caret in the empty paragraph,
/// open the view (with "From your notes" at once) and return the text to read.
pub fn begin(app: &mut WordApp) -> Result<ScopeText, String> {
    let at = stuck::prepare(&mut app.session, app.cowriter.stuck_at.as_ref())?;
    let scope = stuck::scope(&app.session);
    let name = app.ui.assistant.name();
    let notes = stuck::from_notes(&app.session, &scope, &name, &app.ui.assistant.allowed_folders());
    app.cowriter.stuck =
        Some(StuckView { doc: app.session.document_id(), at, name, scope: scope.clone(), questions: Vec::new(), first_line: None, notes });
    crate::mac_ui::set_margin(app, true);
    app.canvas.want_focus = true;
    app.canvas.nav_to_caret = true;
    Ok(scope)
}

/// The run's answer (`cowriter::land` hands it over): the questions and the first line are kept
/// in the view, not landed; the report and refusals stay with the run.
pub fn take(app: &mut WordApp, mut ran: Ran) -> Ran {
    let doc = app.session.document_id();
    if let Some(v) = app.cowriter.stuck.as_mut().filter(|v| v.doc == doc) {
        for o in std::mem::take(&mut ran.outputs) {
            match o {
                Output::Comment { text, .. } => v.questions.push(Question { text, pinned: None }),
                o @ Output::FirstLine { .. } => v.first_line = Some(o),
                _ => {}
            }
        }
    } else {
        ran.outputs.clear();
    }
    ran
}

/// Each frame: the view ends when the caret leaves the paragraph or something is typed in it.
pub fn tick(app: &mut WordApp) {
    let Some(v) = &app.cowriter.stuck else { return };
    if v.doc != app.session.document_id() || !stuck::still_here(&app.session, &v.at) {
        app.cowriter.stuck = None;
    }
}

pub fn active(app: &WordApp) -> bool {
    let doc = app.session.document_id();
    app.cowriter.stuck.as_ref().is_some_and(|v| v.doc == doc)
}

/// The grey italic placeholder at the caret.
pub fn paint_placeholder(app: &WordApp, painter: &egui::Painter, rects: &[Rect], layout: &DocLayout, scale: f32) {
    let Some(v) = app.cowriter.stuck.as_ref().filter(|v| v.doc == app.session.document_id()) else { return };
    let Some(c) = layout.caret_on(&v.at, app.session.page_hint) else { return };
    let (Some(pr), Some(page)) = (rects.get(c.page), layout.pages.get(c.page)) else { return };
    let ps = crate::canvas::page_screen_scale(*pr, page, scale);
    let at = pos2(pr.min.x + c.x * ps, pr.min.y + c.top * ps);
    let size = (c.height * ps / 1.2).clamp(8.0, 40.0);
    // The page stays white in both appearances.
    let grey = Color32::from_rgb(0x8E, 0x8E, 0x93);
    let mut job = egui::text::LayoutJob::single_section(
        tl!(stuck::PLACEHOLDER).to_string(),
        egui::TextFormat { font_id: egui::FontId::proportional(size), color: grey, italics: true, ..Default::default() },
    );
    job.wrap.max_width = (pr.max.x - at.x - 8.0 * ps).max(60.0);
    job.wrap.max_rows = 2;
    let g = painter.layout_job(job);
    painter.galley(at + vec2(4.0, 0.0), g, grey);
}

/// U1–U3 in the right margin at `at`, `w` wide; returns the bottom.
pub fn paint_margin(app: &mut WordApp, ui: &mut Ui, at: Pos2, w: f32) -> f32 {
    let working = app.cowriter.working();
    let Some(v) = app.cowriter.stuck.as_ref() else { return at.y };
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let font = mc::font();
    let pad = 12.0;
    let rect = Rect::from_min_size(pos2(at.x + pad, at.y), vec2(w - pad, 4000.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)).id_salt("stuck_view"));
    let ui = &mut child;
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    let mut acts: Vec<Act> = Vec::new();
    ui.label(egui::RichText::new(tl!("Questions only you can answer")).font(font.clone()).color(m.assistant));
    if v.questions.is_empty() {
        let line = if working { tl!("Thinking of questions…") } else { tl!("No questions this time.") };
        ui.label(egui::RichText::new(line).font(font.clone()).color(t.text_dim));
    }
    for (i, q) in v.questions.iter().enumerate() {
        if q.pinned.is_some() {
            ui.label(egui::RichText::new(format!("{} · {}", q.text, tl!("pinned"))).font(font.clone()).color(t.text_dim));
            continue;
        }
        let r = ui.add(egui::Label::new(egui::RichText::new(&q.text).font(font.clone()).color(t.text)).sense(Sense::click()));
        let r = r.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tl!("Pin as a note at the caret"));
        if r.hovered() {
            ui.painter().hline(r.rect.x_range(), r.rect.max.y, egui::Stroke::new(1.0, t.accent_text));
        }
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, q.text.as_str()));
        if r.clicked() {
            acts.push(Act::Pin(i));
        }
    }
    if !v.notes.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(tl!("From your notes")).font(font.clone()).color(m.assistant));
        for n in &v.notes {
            ui.label(egui::RichText::new(format!("“{}”", n.text)).font(font.clone()).color(t.text));
            ui.label(egui::RichText::new(&n.origin).font(font.clone()).color(t.text_dim));
        }
    }
    if v.first_line.is_some() {
        ui.add_space(8.0);
        let r = ui.add(
            egui::Label::new(egui::RichText::new(tl!("Still stuck? Show a rough first line to argue with")).font(font.clone()).color(t.text_dim))
                .sense(Sense::click()),
        );
        let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
        if r.hovered() {
            ui.painter().hline(r.rect.x_range(), r.rect.max.y, egui::Stroke::new(1.0, t.text_dim));
        }
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Show a rough first line")));
        if r.clicked() {
            acts.push(Act::FirstLine);
        }
    }
    let bottom = ui.min_rect().max.y;
    crate::review_panel::mark_line(ui.painter(), pos2(at.x + 1.0, at.y + 2.0), pos2(at.x + 1.0, bottom), m.assistant, true);
    for a in acts {
        act(app, a);
    }
    bottom + 6.0
}

fn act(app: &mut WordApp, a: Act) {
    let Some(v) = app.cowriter.stuck.as_mut() else { return };
    match a {
        Act::Pin(i) => {
            let Some(text) = v.questions.get(i).map(|q| q.text.clone()) else { return };
            let (name, at) = (v.name.clone(), v.at.clone());
            crate::assistant_link::sync_policy(app);
            match stuck::pin(&mut app.session, &name, &at, &text) {
                Ok(id) => {
                    if let Some(q) = app.cowriter.stuck.as_mut().and_then(|v| v.questions.get_mut(i)) {
                        q.pinned = Some(id);
                    }
                    crate::cowriter::remember_note(app, id, stuck::KIND);
                    app.ui.assistant.remember_name(&name);
                    app.canvas.want_focus = true;
                }
                Err(e) => app.status(e),
            }
        }
        Act::FirstLine => {
            let (Some(line), name, scope) = (v.first_line.take(), v.name.clone(), v.scope.clone()) else { return };
            crate::assistant_link::sync_policy(app);
            let landed = cowriter::apply(&mut app.session, &name, Verb::Stuck, &scope, vec![line]);
            if let Some(why) = landed.lines.first() {
                app.status(why.clone());
            } else {
                app.ui.assistant.remember_name(&name);
            }
            app.canvas.want_focus = true;
        }
    }
}

/// `ui.assistant.stuckView` (for tests and screenshots, no model): `{"answer": "<json text>"}`
/// opens the view and reads that text as the model's answer, through the real reader and guards;
/// `{"pin": i}` and `{"firstLine": true}` click as the person would. Returns the view's state.
pub fn control(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    if let Some(answer) = p.get("answer").and_then(Value::as_str) {
        let scope = begin(app)?;
        let name = app.ui.assistant.name();
        let call = cowriter::Call { session: &app.session, scope: &scope, options: &json!({}), name: &name };
        let ran = cowriter::run(Verb::Stuck, &call, Some(cowriter::Reply { text: answer, stop_reason: Some("stop") }))?;
        let ran = take(app, ran);
        if let Some(msg) = ran.lines.first() {
            app.status(msg.clone());
        }
        if let Some(v) = app.cowriter.stuck.as_mut() {
            v.scope = scope;
        }
    }
    if !active(app) {
        return Err("I'm stuck is not open in this document".into());
    }
    if let Some(i) = p.get("pin").and_then(Value::as_u64) {
        act(app, Act::Pin(usize::try_from(i).unwrap_or(usize::MAX)));
    }
    if p.get("firstLine").and_then(Value::as_bool) == Some(true) {
        act(app, Act::FirstLine);
    }
    Ok(match app.cowriter.stuck.as_ref() {
        Some(v) => json!({
            "open": true,
            "questions": v.questions.iter().map(|q| json!({"text": q.text, "pinned": q.pinned})).collect::<Vec<_>>(),
            "firstLine": v.first_line.is_some(),
            "notes": v.notes.iter().map(|n| json!({"text": n.text, "origin": n.origin})).collect::<Vec<_>>(),
        }),
        None => json!({"open": false}),
    })
}
