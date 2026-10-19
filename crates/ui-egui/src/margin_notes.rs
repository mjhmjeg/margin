//! O3 (OPTIMO.md §4 "Notes in the desk margin"): the co-writer's notes, drawn in the right desk
//! margin beside the page while the margin view is open (the top bar's Co-writer toggle, a quiet
//! marker, or a run that left notes). Shared by every verb that gives comments (Ask, Read as a
//! reader, Check, Research, I'm stuck).
//!
//! * A note is a .docx comment by the AI author (so Word users see it): 268 wide, 12 pt padding
//!   behind a 2 pt dotted `assistant` rule, the kind in the assistant text colour, the note in
//!   `text`, action links in `accent`. Notes sit level with their anchor and are pushed down on
//!   collisions (6 pt gaps). The page draws the anchor itself (assistant tint and dotted mark).
//! * The last run's summary (the model's report, and what was refused or skipped) comes first.
//! * Too narrow a desk: the notes stack in a 320 pt sheet over the right of the window.

use egui::{Pos2, Rect, Sense, Stroke, Ui, pos2};
use serde_json::json;
use wordcraft_doc::Pos;
use wordcraft_layout::DocLayout;

use crate::WordApp;
use crate::mac_controls as mc;
use crate::theme::Tokens;

/// A note's width, its padding behind the rule, the gap between notes.
pub const NOTE_W: f32 = 268.0;
const PAD: f32 = 12.0;
const GAP: f32 = 6.0;
/// The overlay sheet on narrow windows.
pub const SHEET_W: f32 = 320.0;
/// Distance from the page's right edge to the notes.
const FROM_PAGE: f32 = 20.0;

/// An open co-writer note on the page.
struct Note {
    id: u32,
    kind: String,
    text: String,
    anchor: Pos,
    /// Where its anchor sits on screen (y), when laid out.
    y: Option<f32>,
}

/// Whether the document has open notes by an assistant.
pub fn has_notes(app: &WordApp) -> bool {
    app.session.doc.comments.values().any(|c| c.parent.is_none() && !c.resolved && crate::review_panel::is_assistant(&c.author))
}

/// The open notes in document order (top-level comments by an assistant, not resolved).
fn notes(app: &WordApp) -> Vec<Note> {
    let doc = app.session.document_id();
    wordcraft_engine::cmd::review::comment_list(&app.session)
        .into_iter()
        .take(500)
        .filter_map(|(id, pos)| {
            let c = app.session.doc.comments.get(&id)?;
            if c.parent.is_some() || c.resolved || !crate::review_panel::is_assistant(&c.author) {
                return None;
            }
            let text = app.session.doc.plain_text(wordcraft_doc::StoryRef::Part(c.part));
            Some(Note { id, kind: app.cowriter.kind(doc, id).to_string(), text: text.trim().to_string(), anchor: pos?, y: None })
        })
        .collect()
}

/// What a block shows: the kind line, the text, dim lines under it.
struct Block {
    kind: String,
    text: String,
    lines: Vec<String>,
}

/// Draw the margin view. `rects` are the pages on screen, `scale` the zoom.
pub fn paint(app: &mut WordApp, ui: &mut Ui, painter: &egui::Painter, rects: &[Rect], layout: &DocLayout, scale: f32) {
    if !app.cowriter.margin || !crate::mac_ui::active(app) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let clip = ui.clip_rect();
    let mut list = notes(app);
    for n in &mut list {
        if let Some(c) = layout.caret_on(&n.anchor, app.session.page_hint)
            && let (Some(pr), Some(page)) = (rects.get(c.page), layout.pages.get(c.page))
        {
            let ps = crate::canvas::page_screen_scale(*pr, page, scale);
            n.y = Some(pr.min.y + c.top * ps);
        }
    }
    let doc = app.session.document_id();
    let summary =
        app.cowriter.runs(doc).last().filter(|r| !app.cowriter.hide_summary && (!r.report.trim().is_empty() || !r.lines.is_empty())).map(|r| {
            let kind = match r.instruction.trim() {
                "" => tl!(r.verb.label()).to_string(),
                q => format!("“{}”", crate::review_panel::clip(q, 60)),
            };
            Block { kind, text: r.report.clone(), lines: r.lines.clone() }
        });
    let page_right = rects.iter().map(|r| r.max.x).fold(f32::MIN, f32::max);
    // Beside the balloons when people's comments have them.
    let x = page_right + crate::canvas::markup_width(app) * scale + FROM_PAGE;
    let narrow = !page_right.is_finite() || x + NOTE_W + 12.0 > clip.max.x;
    let width = if narrow { SHEET_W - 2.0 * 16.0 } else { NOTE_W };
    let (x, mut y_min) = if narrow {
        let sheet = Rect::from_min_max(pos2(clip.max.x - SHEET_W, clip.min.y), clip.max);
        painter.rect(sheet, 0.0, t.panel, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        // The sheet takes the clicks under it.
        let _ = ui.interact(sheet, ui.id().with("cowriter_sheet"), Sense::click());
        (sheet.min.x + 16.0, sheet.min.y + 16.0)
    } else {
        (x, rects.first().map_or(clip.min.y, |r| r.min.y))
    };
    let mut actions: Vec<Action> = Vec::new();
    if let Some(b) = summary {
        let top = if narrow { y_min } else { y_min.max(clip.min.y + 12.0) };
        y_min = block(ui, painter, pos2(x, top), width, &b, None, &[("Hide", Action::HideSummary)], &mut actions) + GAP * 2.0;
    }
    if list.is_empty() && narrow {
        painter.text(pos2(x, y_min), egui::Align2::LEFT_TOP, tl!("No notes yet."), mc::font(), t.text_dim);
    }
    for n in &list {
        let want = if narrow { y_min } else { n.y.unwrap_or(y_min) };
        let top = want.max(y_min);
        let b = Block { kind: tl!(n.kind.as_str()).to_string(), text: n.text.clone(), lines: Vec::new() };
        let links = [("Done", Action::Done(n.id)), ("Remove", Action::Remove(n.id))];
        y_min = block(ui, painter, pos2(x, top), width, &b, Some((n.id, n.anchor.clone())), &links, &mut actions) + GAP;
    }
    for a in actions {
        match a {
            Action::Done(id) => {
                let _ = app.run("review.resolveComment", json!({"id": id, "value": true}));
            }
            Action::Remove(id) => {
                let _ = app.run("review.deleteComment", json!({"id": id}));
            }
            Action::HideSummary => app.cowriter.hide_summary = true,
            Action::Go(p) => {
                app.session.sel = wordcraft_engine::Selection::caret(app.session.doc.clamp(&p));
                app.canvas.want_focus = true;
            }
        }
    }
}

#[derive(Clone)]
enum Action {
    Done(u32),
    Remove(u32),
    HideSummary,
    Go(Pos),
}

/// One block at `at`, `w` wide: the dotted rule, the kind, the text, dim lines, links. Returns
/// its bottom.
#[allow(clippy::too_many_arguments)]
fn block(
    ui: &mut Ui,
    painter: &egui::Painter,
    at: Pos2,
    w: f32,
    b: &Block,
    note: Option<(u32, Pos)>,
    links: &[(&str, Action)],
    out: &mut Vec<Action>,
) -> f32 {
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let font = mc::font();
    let tw = w - PAD;
    let x = at.x + PAD;
    let mut y = at.y;
    let kind = painter.layout(b.kind.clone(), font.clone(), m.assistant, tw);
    let kind_h = kind.size().y;
    painter.galley(pos2(x, y), kind, m.assistant);
    y += kind_h + 2.0;
    if !b.text.trim().is_empty() {
        let g = painter.layout(b.text.clone(), font.clone(), t.text, tw);
        let r = Rect::from_min_size(pos2(x, y), g.size());
        y += g.size().y + 2.0;
        painter.galley(r.min, g, t.text);
        if let Some((id, anchor)) = &note {
            let resp = ui.interact(r, ui.id().with(("cowriter_note", *id)), Sense::click());
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                out.push(Action::Go(anchor.clone()));
            }
        }
    }
    for l in &b.lines {
        let g = painter.layout(l.clone(), font.clone(), t.text_dim, tw);
        let h = g.size().y;
        painter.galley(pos2(x, y), g, t.text_dim);
        y += h + 2.0;
    }
    let mut lx = x;
    let mut row_h = 0.0f32;
    for (label, action) in links {
        let g = painter.layout_no_wrap(tl!(label).to_string(), font.clone(), t.accent_text);
        let r = Rect::from_min_size(pos2(lx, y + 2.0), g.size());
        row_h = row_h.max(g.size().y + 2.0);
        lx = r.max.x + 14.0;
        let id = ui.id().with(("cowriter_link", *label, note.as_ref().map_or(0, |n| n.0)));
        let resp = ui.interact(r.expand(2.0), id, Sense::click());
        if resp.hovered() {
            painter.hline(r.x_range(), r.max.y, Stroke::new(1.0, t.accent_text));
        }
        painter.galley(r.min, g, t.accent_text);
        let name = tl!(label);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
        if resp.clicked() {
            out.push(action.clone());
        }
    }
    y += row_h;
    crate::review_panel::mark_line(painter, pos2(at.x + 1.0, at.y + 2.0), pos2(at.x + 1.0, y - 2.0), m.assistant, true);
    y
}
