//! Optimo's shared controls (Claude Design handoff §2, Phase 2): one function per control, used by
//! every Optimo screen. Sizes in pt, colours from `Tokens` + `skin::OptimoTokens`.

use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use crate::skin::OptimoTokens;
use crate::theme::{Tokens, regular};

pub const ROW_H: f32 = 28.0;
pub const ROW_GAP: f32 = 8.0;
pub const LABEL_W: f32 = 80.0;
pub const RADIUS: f32 = 6.0;

pub fn font() -> egui::FontId {
    regular(crate::skin::UI_SIZE)
}

fn tokens(ui: &Ui) -> (Tokens, OptimoTokens) {
    (Tokens::get(ui.ctx()), OptimoTokens::get(ui.ctx()))
}

fn shadows(ui: &Ui, r: Rect, radius: f32, list: &[crate::skin::Shadow]) {
    for s in list {
        if s.color.a() > 0 {
            ui.painter().add(s.as_shape(r, radius));
        }
    }
}

/// Keyboard focus: a ring 3 pt outside the control.
fn focus_ring(ui: &Ui, resp: &Response, r: Rect, radius: f32) {
    if resp.has_focus() {
        let (_, m) = tokens(ui);
        ui.painter().rect_stroke(r.expand(1.5), radius + 1.5, Stroke::new(3.0, m.focus_ring), egui::StrokeKind::Outside);
    }
}

// ---------------------------------------------------------------------------------------------
// Toolbar

/// Toolbar button: 32 × 28, radius 6, 17 pt icon; `on` = `checked` fill.
pub fn toolbar_button(ui: &mut Ui, icon: &str, tip: &str, on: bool) -> Response {
    let (t, _) = tokens(ui);
    let (r, resp) = ui.allocate_exact_size(vec2(32.0, 28.0), Sense::click());
    let fill = if on {
        t.checked
    } else if resp.is_pointer_button_down_on() {
        t.pressed
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, RADIUS, fill);
    crate::icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(17.0, 17.0)), icon, t.icon, t.accent);
    focus_ring(ui, &resp, r, RADIUS);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, tl!(tip)));
    resp.on_hover_text(tl!(tip))
}

/// A text button in the toolbar (Format, 100 %): 28 high, 12 side padding.
pub fn toolbar_text_button(ui: &mut Ui, label: &str, on: bool) -> Response {
    let (t, _) = tokens(ui);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font(), t.text);
    let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0, 28.0), Sense::click());
    let fill = if on {
        t.checked
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, RADIUS, fill);
    ui.painter().galley(pos2(r.center().x - galley.size().x / 2.0, r.center().y - galley.size().y / 2.0), galley, t.text);
    focus_ring(ui, &resp, r, RADIUS);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, label));
    resp
}

/// Group divider between toolbar groups: 8 · 1 × 16 `border_strong` · 8.
pub fn toolbar_divider(ui: &mut Ui) {
    let (t, _) = tokens(ui);
    let (r, _) = ui.allocate_exact_size(vec2(17.0, 28.0), Sense::hover());
    ui.painter().vline(r.center().x, r.center().y - 8.0..=r.center().y + 8.0, Stroke::new(1.0, t.border_strong));
}

// ---------------------------------------------------------------------------------------------
// Tabs and segments

/// Underline tabs, centred, 28 apart; returns the clicked index. `height` 44 (40 in sidebars and dialogs).
pub fn underline_tabs(ui: &mut Ui, labels: &[&str], selected: usize, height: f32) -> Option<usize> {
    let (t, _) = tokens(ui);
    let widths: Vec<f32> = labels.iter().map(|l| ui.painter().layout_no_wrap(tl!(*l).to_string(), font(), t.text).size().x).collect();
    let total = widths.iter().sum::<f32>() + 28.0 * labels.len().saturating_sub(1) as f32;
    let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let mut x = area.center().x - total / 2.0;
    let mut clicked = None;
    for (i, (l, w)) in labels.iter().zip(widths).enumerate() {
        let cell = Rect::from_min_size(pos2(x - 6.0, area.min.y), vec2(w + 12.0, height));
        let resp = ui.interact(cell, ui.id().with(("utab", l, i)), Sense::click());
        let on = i == selected;
        let color = if on || resp.hovered() { t.text } else { t.text_dim };
        ui.painter().text(pos2(x, area.center().y), Align2::LEFT_CENTER, tl!(*l), font(), color);
        if on {
            ui.painter().rect_filled(Rect::from_min_max(pos2(x, area.max.y - 2.0), pos2(x + w, area.max.y)), 1.0, t.accent);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, tl!(*l)));
        x += w + 28.0;
    }
    clicked
}

/// What a segment shows.
pub enum Seg<'a> {
    Text(&'a str),
    Icon(&'a str, &'a str),
}

/// Segmented control: 28 high, `segment_track` with an `input_border` outline; the selected
/// segment is a neutral chip (never blue). `selected` may be several (B I U S toggles).
pub fn segmented(ui: &mut Ui, id: &str, items: &[Seg], selected: &[bool]) -> Option<usize> {
    let (t, m) = tokens(ui);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    ui.painter().rect(r, RADIUS, m.segment_track, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let inner = r.shrink(2.0);
    let w = inner.width() / items.len().max(1) as f32;
    let mut clicked = None;
    for (i, item) in items.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(inner.min.x + w * i as f32, inner.min.y), vec2(w, inner.height()));
        let resp = ui.interact(cell, ui.id().with((id, i)), Sense::click());
        let on = selected.get(i).copied().unwrap_or(false);
        if on {
            shadows(ui, cell, 4.0, &m.chip_shadow);
            ui.painter().rect_filled(cell, 4.0, m.segment_chip);
        } else if resp.hovered() {
            ui.painter().rect_filled(cell, 4.0, t.hover.gamma_multiply(0.6));
        }
        let ink = if on { t.text } else { t.text_dim };
        let name = match item {
            Seg::Text(l) => {
                ui.painter().text(cell.center(), Align2::CENTER_CENTER, tl!(*l), font(), ink);
                tl!(*l).to_string()
            }
            Seg::Icon(icon, tip) => {
                crate::icons::paint(ui.painter(), Rect::from_center_size(cell.center(), vec2(16.0, 16.0)), icon, ink, ink);
                tl!(*tip).to_string()
            }
        };
        let resp = if let Seg::Icon(_, tip) = item { resp.on_hover_text(tl!(*tip)) } else { resp };
        if resp.clicked() {
            clicked = Some(i);
        }
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, name.clone()));
    }
    clicked
}

/// [`segmented`] with exactly one selected.
pub fn segmented_one(ui: &mut Ui, id: &str, items: &[Seg], selected: Option<usize>) -> Option<usize> {
    let sel: Vec<bool> = (0..items.len()).map(|i| Some(i) == selected).collect();
    segmented(ui, id, items, &sel)
}

// ---------------------------------------------------------------------------------------------
// Fields

/// Popup button: one control, 28 high, `field_bg` + `input_border`, label left (10 pt in), popup
/// glyph right. Returns the chosen item. Long lists scroll; typing filters when `searchable`.
/// Ask the [`popup_button`] with this `id` to open its list the next time it is drawn.
pub fn request_open(ctx: &egui::Context, id: &str) {
    ctx.data_mut(|d| d.insert_temp(open_request(id), true));
}

fn open_request(id: &str) -> egui::Id {
    egui::Id::new(("popup_button_open", id))
}

pub fn popup_button(ui: &mut Ui, id: &str, width: f32, current: &str, items: &[String], searchable: bool) -> Option<String> {
    let (t, m) = tokens(ui);
    let (r, resp) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
    ui.painter().rect(r, RADIUS, m.field_bg, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let label_w = (r.width() - 34.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(current.to_string(), font(), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(label_w);
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(pos2(r.min.x + 10.0, r.center().y - galley.size().y / 2.0), galley, t.text);
    crate::icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y), vec2(14.0, 14.0)),
        "dropdown",
        t.text_dim,
        t.text_dim,
    );
    focus_ring(ui, &resp, r, RADIUS);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, current));
    if ui.ctx().data_mut(|d| d.remove_temp::<bool>(open_request(id))).is_some() {
        egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&resp));
    }
    let mut chosen = None;
    egui::Popup::menu(&resp).width(width.max(180.0)).show(|ui| {
        let qid = ui.id().with((id, "query"));
        let mut q = ui.data(|d| d.get_temp::<String>(qid)).unwrap_or_default();
        if searchable {
            let te = ui.add(egui::TextEdit::singleline(&mut q).hint_text(tl!("Search")).font(font()).desired_width(f32::INFINITY));
            if te.changed() {
                ui.data_mut(|d| d.insert_temp(qid, q.clone()));
            }
            te.request_focus();
            ui.add_space(4.0);
        }
        let ql = q.to_lowercase();
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for it in items.iter().filter(|it| ql.is_empty() || it.to_lowercase().contains(&ql)) {
                if menu_row(ui, it, it == current).clicked() {
                    chosen = Some(it.clone());
                }
            }
        });
    });
    if chosen.is_some() {
        ui.data_mut(|d| d.remove::<String>(ui.id().with((id, "query"))));
    }
    chosen
}

/// [`popup_button`] with caller-drawn contents: the same 28 high field, but the popup holds
/// whatever `add_contents` puts in it (rows that close the popup on click). For dialogs whose
/// choices are `selectable_value` rows over their own state.
pub fn popup_button_with<R>(ui: &mut Ui, id: egui::Id, width: f32, current: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> Response {
    let (t, m) = tokens(ui);
    let (r, resp) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
    ui.painter().rect(r, RADIUS, m.field_bg, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let label_w = (r.width() - 34.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(current.to_string(), font(), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(label_w);
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(pos2(r.min.x + 10.0, r.center().y - galley.size().y / 2.0), galley, t.text);
    crate::icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y), vec2(14.0, 14.0)),
        "dropdown",
        t.text_dim,
        t.text_dim,
    );
    focus_ring(ui, &resp, r, RADIUS);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, current));
    egui::Popup::menu(&resp).id(id).width(width.max(180.0)).show(|ui| {
        egui::ScrollArea::vertical().max_height(320.0).show(ui, add_contents);
    });
    resp
}

/// Number field: 28 high, value right-aligned in `text`, unit after it in `text_dim` ("28 pt").
/// Drag or type; returns the new value once the edit is done.
pub fn number_field(ui: &mut Ui, id: &str, value: f32, step: f32, range: std::ops::RangeInclusive<f32>, unit: &str) -> Option<f32> {
    let (t, m) = tokens(ui);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    ui.painter().rect(r, RADIUS, m.field_bg, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let unit_w = if unit.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(unit.to_string(), font(), t.text_dim).size().x + 6.0 };
    if !unit.is_empty() {
        ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, unit, font(), t.text_dim);
    }
    let field = Rect::from_min_max(pos2(r.min.x + 8.0, r.min.y + 2.0), pos2(r.max.x - 10.0 - unit_w, r.max.y - 2.0));
    let shown = |n: f32| {
        let s = format!("{n:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let edit_id = ui.id().with((id, "edit"));
    let mut editing = ui.data(|d| d.get_temp::<String>(edit_id));
    let mut out = None;
    if let Some(text) = editing.as_mut() {
        // Typing: a frameless, right-aligned text field; Enter or leaving it commits.
        let te = ui.put(
            field,
            egui::TextEdit::singleline(text).frame(egui::Frame::NONE).font(font()).horizontal_align(egui::Align::RIGHT).text_color(t.text),
        );
        if !te.has_focus() && !te.lost_focus() {
            te.request_focus();
        }
        focus_ring(ui, &te, r, RADIUS);
        if te.lost_focus() {
            if let Ok(n) = text.trim().replace(',', ".").parse::<f32>() {
                let n = n.clamp(*range.start(), *range.end());
                if (n - value).abs() > f32::EPSILON {
                    out = Some(n);
                }
            }
            ui.data_mut(|d| d.remove::<String>(edit_id));
        } else {
            let t2 = text.clone();
            ui.data_mut(|d| d.insert_temp(edit_id, t2));
        }
        return out;
    }
    // Showing: the value right-aligned; drag sideways to change it, click to type.
    let resp = ui.interact(field, edit_id.with("drag"), Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let drag_id = edit_id.with("value");
    let mut v = ui.data(|d| d.get_temp::<f32>(drag_id)).filter(|_| resp.dragged()).unwrap_or(value);
    if resp.dragged() {
        v = (v + resp.drag_delta().x * step).clamp(*range.start(), *range.end());
        ui.data_mut(|d| d.insert_temp(drag_id, v));
    }
    ui.painter().text(pos2(field.max.x, field.center().y), Align2::RIGHT_CENTER, shown(v), font(), t.text);
    if resp.clicked() {
        ui.data_mut(|d| d.insert_temp(edit_id, shown(value)));
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<f32>(drag_id));
        if (v - value).abs() > f32::EPSILON {
            out = Some((v * 100.0).round() / 100.0);
        }
    }
    focus_ring(ui, &resp, r, RADIUS);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, format!("{} {unit}", shown(value))));
    out
}

/// [`number_field`] with the caller in charge of text: `display` writes the value shown ("No limit",
/// a decimal comma), `edit` writes the text the field starts typing from, `parse` reads what was
/// typed (and may reject it). The result is always clamped to `range`; a non-finite parse is ignored.
#[allow(clippy::too_many_arguments)]
pub fn number_field_with(
    ui: &mut Ui,
    id: &str,
    value: f32,
    step: f32,
    range: std::ops::RangeInclusive<f32>,
    unit: &str,
    display: &dyn Fn(f32) -> String,
    edit: &dyn Fn(f32) -> String,
    parse: &dyn Fn(&str) -> Option<f32>,
) -> Option<f32> {
    let (t, m) = tokens(ui);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    ui.painter().rect(r, RADIUS, m.field_bg, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let unit_w = if unit.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(unit.to_string(), font(), t.text_dim).size().x + 6.0 };
    if !unit.is_empty() {
        ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, unit, font(), t.text_dim);
    }
    let field = Rect::from_min_max(pos2(r.min.x + 8.0, r.min.y + 2.0), pos2(r.max.x - 10.0 - unit_w, r.max.y - 2.0));
    let edit_id = ui.id().with((id, "edit"));
    let mut editing = ui.data(|d| d.get_temp::<String>(edit_id));
    let mut out = None;
    if let Some(text) = editing.as_mut() {
        // Typing: a frameless, right-aligned text field; Enter or leaving it commits.
        let te = ui.put(
            field,
            egui::TextEdit::singleline(text).frame(egui::Frame::NONE).font(font()).horizontal_align(egui::Align::RIGHT).text_color(t.text),
        );
        if !te.has_focus() && !te.lost_focus() {
            te.request_focus();
        }
        focus_ring(ui, &te, r, RADIUS);
        if te.lost_focus() {
            if let Some(n) = parse(text.trim()).filter(|n| n.is_finite()) {
                let n = n.clamp(*range.start(), *range.end());
                if (n - value).abs() > f32::EPSILON {
                    out = Some(n);
                }
            }
            ui.data_mut(|d| d.remove::<String>(edit_id));
        } else {
            let t2 = text.clone();
            ui.data_mut(|d| d.insert_temp(edit_id, t2));
        }
        return out;
    }
    // Showing: the value right-aligned; drag sideways to change it, click to type.
    let resp = ui.interact(field, edit_id.with("drag"), Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let drag_id = edit_id.with("value");
    let mut v = ui.data(|d| d.get_temp::<f32>(drag_id)).filter(|_| resp.dragged()).unwrap_or(value);
    if resp.dragged() {
        v = (v + resp.drag_delta().x * step).clamp(*range.start(), *range.end());
        ui.data_mut(|d| d.insert_temp(drag_id, v));
    }
    ui.painter().text(pos2(field.max.x, field.center().y), Align2::RIGHT_CENTER, display(v), font(), t.text);
    if resp.clicked() {
        ui.data_mut(|d| d.insert_temp(edit_id, edit(value)));
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<f32>(drag_id));
        if (v - value).abs() > f32::EPSILON {
            out = Some((v * 100.0).round() / 100.0);
        }
    }
    focus_ring(ui, &resp, r, RADIUS);
    resp.widget_info(|| egui::WidgetInfo {
        current_text_value: Some(format!("{} {unit}", display(value)).trim().to_string()),
        value: Some(f64::from(value)),
        ..egui::WidgetInfo::new(egui::WidgetType::DragValue)
    });
    out
}

/// A labelled row: label column 80 (`text_dim`), 8 gap, the control fills the rest.
pub fn labelled_row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (t, _) = tokens(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, ROW_H), Sense::hover());
        ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, tl!(label), font(), t.text_dim);
        add(ui)
    })
    .inner
}

/// A section: 20 top/bottom, 16 sides, title in `section_title`, hairline `border` below.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let (t, m) = tokens(ui);
    let resp = egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 16, top: 20, bottom: 20 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        ui.label(egui::RichText::new(tl!(title)).font(font()).color(m.section_title));
        ui.add_space(10.0 - ROW_GAP);
        add(ui);
    });
    let r = resp.response.rect;
    ui.painter().hline(r.x_range(), r.max.y, Stroke::new(1.0, t.border));
}

/// Hint text under a control.
pub fn hint(ui: &mut Ui, text: &str) {
    let (_, m) = tokens(ui);
    ui.label(egui::RichText::new(tl!(text)).font(font()).color(m.hint));
}

// ---------------------------------------------------------------------------------------------
// Toggles and buttons

/// Checkbox (14, radius 4) with its label; returns true when clicked.
pub fn checkbox(ui: &mut Ui, label: &str, on: bool) -> bool {
    toggle_mark(ui, label, on, false).clicked()
}

/// [`checkbox`] for callers that need the response (dialogs: `changed()`).
pub fn checkbox_response(ui: &mut Ui, label: &str, on: bool) -> Response {
    toggle_mark(ui, label, on, false)
}

/// Radio (14, round) with its label; returns true when clicked.
pub fn radio(ui: &mut Ui, label: &str, on: bool) -> bool {
    toggle_mark(ui, label, on, true).clicked()
}

/// [`radio`] for callers that need the response.
pub fn radio_response(ui: &mut Ui, label: &str, on: bool) -> Response {
    toggle_mark(ui, label, on, true)
}

fn toggle_mark(ui: &mut Ui, label: &str, on: bool, round: bool) -> Response {
    let (t, m) = tokens(ui);
    let text_w = ui.painter().layout_no_wrap(tl!(label).to_string(), font(), t.text).size().x;
    let (r, resp) = ui.allocate_exact_size(vec2(14.0 + 8.0 + text_w, ROW_H), Sense::click());
    let b = Rect::from_center_size(pos2(r.min.x + 7.0, r.center().y), vec2(14.0, 14.0));
    let radius = if round { 7.0 } else { 4.0 };
    if on {
        ui.painter().rect_filled(b, radius, t.accent);
        if round {
            ui.painter().circle_filled(b.center(), 3.0, t.on_accent);
        } else {
            let s = Stroke::new(1.75, t.on_accent);
            ui.painter().line_segment([pos2(b.min.x + 3.5, b.center().y), pos2(b.min.x + 6.0, b.max.y - 4.0)], s);
            ui.painter().line_segment([pos2(b.min.x + 6.0, b.max.y - 4.0), pos2(b.max.x - 3.0, b.min.y + 4.0)], s);
        }
    } else {
        ui.painter().rect(b, radius, m.field_bg, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    }
    ui.painter().text(pos2(b.max.x + 8.0, r.center().y), Align2::LEFT_CENTER, tl!(label), font(), t.text);
    focus_ring(ui, &resp, b, radius);
    let wt = if round { egui::WidgetType::RadioButton } else { egui::WidgetType::Checkbox };
    resp.widget_info(|| egui::WidgetInfo::selected(wt, true, on, tl!(label)));
    resp
}

/// Switch 30 × 18, knob 14: on = `accent`, off = `segment_track` + `input_border` ring.
pub fn switch(ui: &mut Ui, label: &str, on: bool) -> bool {
    let (t, m) = tokens(ui);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, tl!(label), font(), t.text);
    let s = Rect::from_min_size(pos2(r.max.x - 30.0, r.center().y - 9.0), vec2(30.0, 18.0));
    if on {
        ui.painter().rect_filled(s, 9.0, t.accent);
    } else {
        ui.painter().rect(s, 9.0, m.segment_track, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    }
    let kx = if on { s.max.x - 9.0 } else { s.min.x + 9.0 };
    ui.painter().circle_filled(pos2(kx, s.center().y), 7.0, if on { t.on_accent } else { Color32::WHITE });
    ui.painter().circle_stroke(pos2(kx, s.center().y), 7.0, Stroke::new(0.5, Color32::from_black_alpha(40)));
    focus_ring(ui, &resp, s, 9.0);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, tl!(label)));
    resp.clicked()
}

/// Button: 28 high, min 80 wide, radius 6, 16 side padding. Primary = `accent`; else secondary.
pub fn button(ui: &mut Ui, label: &str, primary: bool, enabled: bool) -> Response {
    let (t, m) = tokens(ui);
    let w = (ui.painter().layout_no_wrap(tl!(label).to_string(), font(), t.text).size().x + 32.0).max(80.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), sense);
    let (fill, stroke, ink) = if primary { (t.accent, Stroke::NONE, t.on_accent) } else { (m.field_bg, Stroke::new(1.0, t.input_border), t.text) };
    let fill = if resp.is_pointer_button_down_on() && enabled { if primary { t.accent_text } else { t.pressed } } else { fill };
    let (fill, ink) = if enabled { (fill, ink) } else { (fill.gamma_multiply(0.5), ink.gamma_multiply(0.5)) };
    ui.painter().rect(r, RADIUS, fill, stroke, egui::StrokeKind::Inside);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, tl!(label), font(), ink);
    focus_ring(ui, &resp, r, RADIUS);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tl!(label)));
    resp
}

/// Link-style action in `accent` (Font…, Edit).
pub fn link(ui: &mut Ui, label: &str) -> Response {
    let (t, _) = tokens(ui);
    let resp = ui.add(egui::Label::new(egui::RichText::new(tl!(label)).font(font()).color(t.accent)).sense(Sense::click()));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------------------------------------------------------------------------------------
// Menus and popovers

/// A menu row: 26 high, radius 5; hover = `accent` pill + `on_accent` text; the current value
/// gets a check in a 20 pt left column.
pub fn menu_row(ui: &mut Ui, label: &str, current: bool) -> Response {
    let (t, _) = tokens(ui);
    // Menus give unbounded width; keep rows to a sane span.
    let w = ui.available_width();
    let w = if w.is_finite() { w.clamp(160.0, 360.0) } else { 200.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    let hot = resp.hovered();
    if hot {
        ui.painter().rect_filled(r, 5.0, t.accent);
    }
    let ink = if hot { t.on_accent } else { t.text };
    if current {
        crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 11.0, r.center().y), vec2(13.0, 13.0)), "check", ink, ink);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), font(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(r.width() - 28.0);
    let g = ui.painter().layout_job(job);
    ui.painter().galley(pos2(r.min.x + 20.0, r.center().y - g.size().y / 2.0), g, ink);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, current, label));
    resp
}

/// Popover / menu chrome (`popover_*`, radius 8, padding 6), applied to egui's popups and windows
/// while the skin is on. Called from `theme::apply`.
pub fn style_popovers(style: &mut egui::Style, dark: bool) {
    let m = if dark { OptimoTokens::dark() } else { OptimoTokens::light() };
    let v = &mut style.visuals;
    v.window_fill = m.popover_bg;
    v.window_stroke = Stroke::new(1.0, m.popover_border);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.popup_shadow = m.popover_shadow;
    v.window_shadow = m.popover_shadow;
    style.spacing.menu_margin = egui::Margin::same(6);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Click the number field, type a value, press Enter: the new value comes back once.
    #[test]
    fn number_field_commits_typed_value() {
        use egui_kittest::kittest::Queryable;
        let mut got: Option<f32> = None;
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(300.0, 60.0)).build_ui_state(
            |ui, got: &mut Option<f32>| {
                if let Some(v) = number_field(ui, "n", 12.0, 1.0, 1.0..=400.0, "pt") {
                    *got = Some(v);
                }
            },
            got.take(),
        );
        h.run();
        h.get_by_label("12 pt").click();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.run();
        h.get_by_role(egui::accesskit::Role::TextInput).type_text("14");
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(*h.state(), Some(14.0));
    }

    #[test]
    fn segmented_reports_the_click_and_draws_a_neutral_chip() {
        use egui_kittest::kittest::Queryable;
        // A bare Context has no font atlas yet and panics in epaint; the kittest harness sets one up.
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(300.0, 60.0)).build_ui_state(
            |ui, clicked: &mut Option<usize>| {
                if let Some(i) = segmented_one(ui, "t", &[Seg::Text("A"), Seg::Text("B")], Some(1)) {
                    *clicked = Some(i);
                }
            },
            None,
        );
        h.run();
        assert_eq!(*h.state(), None);
        h.get_by_label("A").click();
        h.run();
        assert_eq!(*h.state(), Some(0));
        // The chip is the neutral `segment_chip`, never the accent (HANDOFF §1).
        assert_ne!(OptimoTokens::light().segment_chip, Tokens::light().accent);
    }
}
