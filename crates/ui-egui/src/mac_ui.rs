//! Margin's Mac layout: one slim toolbar in the title-bar band, a Format inspector on the right
//! (Text / Layout / Document) and the Navigation pane as the outline sidebar. The ribbon stays
//! reachable as View › Classic Ribbon (⌃⌘R). Every control runs an existing engine command.

use egui::{Align2, Color32, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::WordApp;
use crate::chrome::{MAC_CONTENT_LEFT, MAC_TITLE_BAR};
use crate::mac_controls as mc;
use crate::theme::{Tokens, regular};
use crate::widgets::run_from_ribbon;

/// Margin's interface settings, saved with the other preferences in `ui.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MarginPrefs {
    pub classic_ribbon: bool,
    pub inspector: bool,
    pub tab: InspectorTab,
    pub text_width: TextWidth,
}

impl Default for MarginPrefs {
    fn default() -> Self {
        MarginPrefs { classic_ribbon: false, inspector: true, tab: InspectorTab::Text, text_width: TextWidth::Medium }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InspectorTab {
    #[default]
    Text,
    Layout,
    Document,
}

/// Pageless text width (Google Docs' Narrow / Medium / Full).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextWidth {
    Narrow,
    #[default]
    Medium,
    Full,
}

impl TextWidth {
    /// Width of the pageless "page" in points (`None`: as wide as the window).
    pub fn points(self) -> Option<f32> {
        match self {
            TextWidth::Narrow => Some(520.0),
            TextWidth::Medium => Some(680.0),
            TextWidth::Full => None,
        }
    }
}

/// Whether the Mac layout replaces the ribbon this frame.
pub fn active(app: &WordApp) -> bool {
    crate::skin::on() && !app.ui.margin.classic_ribbon
}

/// Pageless is upstream's Web Layout view.
pub fn pageless(app: &WordApp) -> bool {
    app.session.view.mode == wordcraft_layout::ViewMode::Web
}

const CM: f32 = 72.0 / 2.54;
const PAD: f32 = 16.0;
const ROW_H: f32 = mc::ROW_H;
const INSPECTOR_W: f32 = 272.0;

fn font() -> egui::FontId {
    regular(crate::skin::UI_SIZE)
}

// ---------------------------------------------------------------------------------------------
// Toolbar

/// The toolbar: shares the 28 pt band with the traffic lights (AppKit fixes them there), plus a
/// little air below so the controls don't touch the document.
pub fn toolbar(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let mac = app.integrated_titlebar;
    let left = if mac { MAC_CONTENT_LEFT } else { 10.0 };
    egui::Panel::top("margin_toolbar")
        .exact_size(MAC_TITLE_BAR + 8.0)
        .frame(egui::Frame::NONE.fill(t.title_bar).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            let full = ui.max_rect();
            let drag = ui.interact(full, ui.id().with("toolbar_drag"), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            // Controls centre on the traffic lights' band.
            let band = Rect::from_min_max(pos2(full.min.x + left, full.min.y + 3.0), pos2(full.max.x - 10.0, full.min.y + MAC_TITLE_BAR + 3.0));
            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(band).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let ui = &mut ui;
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);

            let nav = app.session.view.nav_pane;
            if tool(ui, app, "navPane", "Show outline", nav).clicked() {
                run_from_ribbon(app, "view.navigationPane", json!({}));
            }
            ui.add_space(8.0);
            let stem = app.title_stem();
            ui.label(egui::RichText::new(stem).font(font()).color(t.text));
            if app.session.dirty {
                ui.label(egui::RichText::new(format!("— {}", tl!("Edited"))).font(font()).color(t.text_dim));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                let open = app.ui.margin.inspector;
                if text_tool(ui, "Format", open).on_hover_text(tl!("Show or hide the Format inspector")).clicked() {
                    app.ui.margin.inspector = !open;
                }
                zoom_menu(ui, app);
                ui.add_space(12.0);
                more_menu(ui, app);
                let tracking = app.session.doc.settings.track_changes;
                if tool(ui, app, "trackChanges", "Track changes", tracking).clicked() {
                    run_from_ribbon(app, "review.trackChanges", json!({}));
                }
                ui.add_space(8.0);
                if tool(ui, app, "newComment", "Comment", false).clicked() {
                    run_from_ribbon(app, "review.newComment", json!({}));
                }
                shape_menu(ui, app);
                if tool(ui, app, "picture", "Image", false).clicked() {
                    run_from_ribbon(app, "insert.picture", json!({}));
                }
                table_menu(ui, app);
                ui.add_space(8.0);
                // The style box lays out left to right inside the right-to-left toolbar group.
                let st = app.session.run("format.state", &json!({})).unwrap_or_default();
                ui.allocate_ui_with_layout(vec2(150.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    style_popup(ui, app, &st, 150.0)
                });
            });
        });
}

/// An icon button in the toolbar; `on` draws it toggled.
fn tool(ui: &mut Ui, _app: &WordApp, icon: &str, tip: &str, on: bool) -> Response {
    mc::toolbar_button(ui, icon, tip, on)
}

/// A text button in the toolbar (Format, zoom).
fn text_tool(ui: &mut Ui, label: &str, on: bool) -> Response {
    mc::toolbar_text_button(ui, tl!(label), on)
}

fn table_menu(ui: &mut Ui, app: &mut WordApp) {
    let resp = tool(ui, app, "table", "Table", false);
    egui::Popup::menu(&resp).width(200.0).show(|ui| {
        ui.set_min_width(160.0);
        for (r, c) in [(2, 2), (3, 3), (4, 4), (5, 3)] {
            if mc::menu_row(ui, &format!("{r} × {c}"), false).clicked() {
                run_from_ribbon(app, "insert.table", json!({"rows": r, "cols": c}));
            }
        }
        ui.separator();
        if mc::menu_row(ui, tl!("More…"), false).clicked() {
            run_from_ribbon(app, "insert.table", json!({}));
        }
    });
}

fn shape_menu(ui: &mut Ui, app: &mut WordApp) {
    let resp = tool(ui, app, "shapes", "Shape", false);
    egui::Popup::menu(&resp).width(200.0).show(|ui| {
        ui.set_min_width(160.0);
        for (label, kind) in
            [("Rectangle", "rectangle"), ("Rounded rectangle", "roundedRectangle"), ("Oval", "ellipse"), ("Line", "line"), ("Arrow", "arrow")]
        {
            if mc::menu_row(ui, tl!(label), false).clicked() {
                run_from_ribbon(app, "insert.shape", json!({"kind": kind}));
            }
        }
    });
}

fn zoom_menu(ui: &mut Ui, app: &mut WordApp) {
    let pct = (app.session.view.zoom * 100.0).round();
    let resp = text_tool(ui, &format!("{pct:.0}%"), false);
    egui::Popup::menu(&resp).width(200.0).show(|ui| {
        ui.set_min_width(120.0);
        for z in [75, 100, 125, 150, 200] {
            if mc::menu_row(ui, &format!("{z}%"), (pct - z as f32).abs() < 0.5).clicked() {
                run_from_ribbon(app, "view.zoom", json!({"value": z}));
            }
        }
        ui.separator();
        if mc::menu_row(ui, tl!("Fit width"), false).clicked() {
            run_from_ribbon(app, "view.zoom", json!({"value": "pageWidth"}));
        }
    });
}

/// "⋯": things that move to the menu bar in phase 4.
fn more_menu(ui: &mut Ui, app: &mut WordApp) {
    let resp = tool(ui, app, "more", "More", false);
    egui::Popup::menu(&resp).width(200.0).show(|ui| {
        ui.set_min_width(200.0);
        if mc::menu_row(ui, tl!("Classic ribbon  ⌃⌘R"), false).clicked() {
            app.ui.margin.classic_ribbon = true;
        }
        if mc::menu_row(ui, tl!("File…"), false).clicked() {
            app.ui.backstage = true;
        }
        if mc::menu_row(ui, tl!("Search commands"), false).clicked() {
            let _ = app.run("ui.dialog", json!({"name": "commands"}));
        }
    });
}

/// ⌃⌘R switches between the Mac layout and the Classic ribbon, both ways.
pub fn shortcuts(app: &mut WordApp, ctx: &egui::Context) {
    if !crate::skin::on() {
        return;
    }
    let hit = ctx.input_mut(|i| i.consume_key(egui::Modifiers { ctrl: true, mac_cmd: true, command: true, ..Default::default() }, egui::Key::R));
    if hit {
        app.ui.margin.classic_ribbon = !app.ui.margin.classic_ribbon;
    }
}

// ---------------------------------------------------------------------------------------------
// Inspector

pub fn inspector(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::right("margin_inspector")
        .exact_size(INSPECTOR_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
            ui.add_space(12.0);
            pad(ui, |ui| {
                let tabs = [(InspectorTab::Text, "Text"), (InspectorTab::Layout, "Layout"), (InspectorTab::Document, "Document")];
                let cur = app.ui.margin.tab;
                if let Some(i) = segmented(ui, &tabs.map(|(_, l)| l), tabs.iter().position(|(k, _)| *k == cur)) {
                    app.ui.margin.tab = tabs.get(i).map_or(cur, |(k, _)| *k);
                }
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.ui.margin.tab {
                InspectorTab::Text => text_tab(app, ui),
                InspectorTab::Layout => layout_tab(app, ui),
                InspectorTab::Document => document_tab(app, ui),
            });
        });
}

fn text_tab(app: &mut WordApp, ui: &mut Ui) {
    let st = app.session.run("format.state", &json!({})).unwrap_or_default();
    let flag = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
    let rp = app.session.doc.para_at(&app.session.sel.focus).map(|p| app.session.doc.styles.resolve_para(&p.props));

    section(ui, "Paragraph style", |ui| style_popup(ui, app, &st, ui.available_width()));

    section(ui, "Font", |ui| {
        let w = ui.available_width();
        let font_name = st.get("font").and_then(Value::as_str).unwrap_or("").to_string();
        let shown = app.previews.font_shown(&font_name);
        let fams = app.previews.families();
        if let Some(f) = mc::popup_button(ui, "margin_font", w, &shown, &fams, true) {
            if f == crate::widgets::ADD_FONT {
                run_from_ribbon(app, "font.add", json!({}));
            } else {
                let f = app.previews.font_stored(&f);
                run_from_ribbon(app, "format.font", json!({"name": f}));
            }
        }
        ui.horizontal(|ui| {
            label(ui, "Size", 64.0);
            let size = st.get("size").and_then(Value::as_f64).unwrap_or(12.0) as f32;
            if let Some(v) = number(ui, "margin_size", size, 1.0, 1.0..=400.0, "pt") {
                run_from_ribbon(app, "format.size", json!({"size": v}));
            }
        });
        let marks = [
            ("B", "format.bold", flag("bold")),
            ("I", "format.italic", flag("italic")),
            ("U", "format.underline", flag("underline")),
            ("S", "format.strikethrough", flag("strike")),
        ];
        if let Some(i) = toggles(ui, &marks.map(|(l, _, on)| (l, on)))
            && let Some((_, id, _)) = marks.get(i)
        {
            run_from_ribbon(app, id, json!({}));
        }
    });

    section(ui, "Alignment", |ui| {
        use wordcraft_doc::Align as A;
        let align = rp.as_ref().map(|r| r.align.visual(r.bidi)).unwrap_or_default();
        let opts = [("Left", A::Left, "left"), ("Center", A::Center, "center"), ("Right", A::Right, "right"), ("Justify", A::Justify, "justify")];
        if let Some(i) = segmented(ui, &opts.map(|(l, _, _)| l), opts.iter().position(|(_, a, _)| *a == align))
            && let Some((_, _, v)) = opts.get(i)
        {
            run_from_ribbon(app, "para.align", json!({"value": v}));
        }
    });

    section(ui, "Spacing", |ui| {
        let (before, after, line) = rp.as_ref().map_or((0.0, 0.0, 1.0), |r| (r.space_before, r.space_after, line_multiple(&r.line_spacing)));
        ui.horizontal(|ui| {
            label(ui, "Line", 64.0);
            if let Some(v) = number(ui, "margin_line", line, 0.05, 0.5..=5.0, "×") {
                run_from_ribbon(app, "para.lineSpacing", json!({"value": v}));
            }
        });
        ui.horizontal(|ui| {
            label(ui, "Before", 64.0);
            if let Some(v) = number(ui, "margin_before", before, 1.0, 0.0..=400.0, "pt") {
                run_from_ribbon(app, "para.spacing", json!({"before": v}));
            }
        });
        ui.horizontal(|ui| {
            label(ui, "After", 64.0);
            if let Some(v) = number(ui, "margin_after", after, 1.0, 0.0..=400.0, "pt") {
                run_from_ribbon(app, "para.spacing", json!({"after": v}));
            }
        });
    });

    section(ui, "Bullets and lists", |ui| {
        let lists = wordcraft_engine::cmd::para::list_buttons(&app.session.doc, rp.as_ref().and_then(|r| r.numbering));
        let cur = if lists.bullets {
            1
        } else if lists.numbering {
            2
        } else {
            0
        };
        if let Some(i) = segmented(ui, &["None", "Bullets", "Numbers"], Some(cur)) {
            match i {
                1 => run_from_ribbon(app, "para.bullets", json!({})),
                2 => run_from_ribbon(app, "para.numbering", json!({})),
                _ if lists.bullets => run_from_ribbon(app, "para.bullets", json!({"off": true})),
                _ => run_from_ribbon(app, "para.numbering", json!({"off": true})),
            }
        }
    });
}

fn layout_tab(app: &mut WordApp, ui: &mut Ui) {
    let rp = app.session.doc.para_at(&app.session.sel.focus).map(|p| app.session.doc.styles.resolve_para(&p.props));
    let (left, right, first) = rp.as_ref().map_or((0.0, 0.0, 0.0), |r| (r.indent_left, r.indent_right, r.indent_first));
    section(ui, "Indents", |ui| {
        for (name, key, v) in [("Left", "left", left), ("Right", "right", right), ("First line", "firstLine", first)] {
            ui.horizontal(|ui| {
                label(ui, name, 80.0);
                if let Some(cm) = number(ui, &format!("margin_ind_{key}"), v / CM, 0.1, -20.0..=50.0, "cm") {
                    run_from_ribbon(app, "para.indents", json!({ key: cm * CM }));
                }
            });
        }
    });
    section(ui, "Columns", |ui| {
        let count = caret_section(app).map_or(1, |s| s.columns.count as usize);
        if let Some(i) = segmented(ui, &["1", "2", "3"], Some(count.clamp(1, 3) - 1)) {
            run_from_ribbon(app, "layout.columns", json!({"count": i + 1}));
        }
    });
    section(ui, "Pagination", |ui| {
        let (keep_next, keep_lines) = rp.as_ref().map_or((false, false), |r| (r.keep_next, r.keep_lines));
        if check(ui, "Keep with next", keep_next) {
            run_from_ribbon(app, "para.keepNext", json!({}));
        }
        if check(ui, "Keep lines together", keep_lines) {
            run_from_ribbon(app, "para.keepLines", json!({}));
        }
    });
}

fn document_tab(app: &mut WordApp, ui: &mut Ui) {
    let pageless = pageless(app);
    section(ui, "Format", |ui| {
        if let Some(i) = segmented(ui, &["Pages", "Pageless"], Some(usize::from(pageless))) {
            set_pageless(app, i == 1);
        }
        if pageless {
            ui.label(egui::RichText::new(tl!("Text width")).font(font()).color(Tokens::get(ui.ctx()).text_dim));
            let opts = [(TextWidth::Narrow, "Narrow"), (TextWidth::Medium, "Medium"), (TextWidth::Full, "Full")];
            let cur = app.ui.margin.text_width;
            if let Some(i) = segmented(ui, &opts.map(|(_, l)| l), opts.iter().position(|(w, _)| *w == cur)) {
                app.ui.margin.text_width = opts.get(i).map_or(cur, |(w, _)| *w);
            }
        }
        hint(ui, "Pageless hides page breaks, headers and footers. They come back when you print or export to PDF.");
    });
    if pageless {
        return;
    }
    let Some(sect) = caret_section(app) else { return };
    section(ui, "Page", |ui| {
        let (w, h) = (sect.page_w, sect.page_h);
        let name = page_name(w.min(h), w.max(h));
        let names = ["A4", "A5", "Letter", "Legal"].map(String::from);
        if let Some(n) = mc::popup_button(ui, "margin_page", ui.available_width(), name, &names, false) {
            run_from_ribbon(app, "layout.size", json!({"name": n}));
        }
        let landscape = w > h;
        if let Some(i) = segmented(ui, &["Portrait", "Landscape"], Some(usize::from(landscape))) {
            run_from_ribbon(app, "layout.orientation", json!({"value": if i == 1 { "landscape" } else { "portrait" }}));
        }
    });
    section(ui, "Margins", |ui| {
        for (name, key, v) in [
            ("Top", "top", sect.margin_top),
            ("Bottom", "bottom", sect.margin_bottom),
            ("Left", "left", sect.margin_left),
            ("Right", "right", sect.margin_right),
        ] {
            ui.horizontal(|ui| {
                label(ui, name, 80.0);
                if let Some(cm) = number(ui, &format!("margin_m_{key}"), v / CM, 0.1, 0.0..=20.0, "cm") {
                    run_from_ribbon(app, "layout.margins", json!({ key: cm * CM }));
                }
            });
        }
    });
    section(ui, "Headers and footers", |ui| {
        if button(ui, "Edit header") {
            run_from_ribbon(app, "insert.header", json!({"preset": "blank"}));
        }
        if button(ui, "Edit footer") {
            run_from_ribbon(app, "insert.footer", json!({"preset": "blank"}));
        }
        if button(ui, "Add page numbers") {
            run_from_ribbon(app, "insert.pageNumber", json!({"position": "bottom", "align": "center"}));
        }
    });
}

/// Pages ↔ Pageless. Pageless is Web Layout at 100% (a fitted zoom would stretch the narrow column).
pub fn set_pageless(app: &mut WordApp, on: bool) {
    if on {
        run_from_ribbon(app, "view.webLayout", json!({}));
        run_from_ribbon(app, "view.zoom", json!({"value": 100}));
    } else {
        run_from_ribbon(app, "view.printLayout", json!({}));
    }
}

/// Pageless reads as one continuous sheet: the desk around the column takes the page's colour.
pub fn canvas_fill(app: &WordApp, desk: Color32) -> Color32 {
    if !active(app) || !pageless(app) {
        return desk;
    }
    if app.session.view.dark_mode { Color32::from_gray(0x1E) } else { Color32::WHITE }
}

/// Before the canvas lays out: the pageless column width for the chosen Text width.
pub fn before_canvas(app: &mut WordApp, avail_w: f32) {
    if !active(app) || !pageless(app) {
        return;
    }
    let px_per_pt = app.session.view.zoom.max(0.1) * crate::canvas::PX_PER_PT;
    let full = (avail_w / px_per_pt - 48.0).max(200.0);
    let w = app.ui.margin.text_width.points().map_or(full, |p| p.min(full));
    if (app.session.view.web_width - w).abs() > 0.5 {
        app.session.view.web_width = w;
    }
}

fn line_multiple(l: &wordcraft_doc::props::LineSpacing) -> f32 {
    match l {
        wordcraft_doc::props::LineSpacing::Multiple(m) => *m,
        _ => 1.0,
    }
}

fn page_name(w: f32, h: f32) -> &'static str {
    let near = |a: f32, b: f32| (a - b).abs() < 3.0;
    if near(w, 595.0) && near(h, 842.0) {
        "A4"
    } else if near(w, 420.0) && near(h, 595.0) {
        "A5"
    } else if near(w, 612.0) && near(h, 792.0) {
        "Letter"
    } else if near(w, 612.0) && near(h, 1008.0) {
        "Legal"
    } else {
        "Custom"
    }
}

/// The section the caret is in (body text; headers, notes and text boxes use the first).
fn caret_section(app: &WordApp) -> Option<wordcraft_doc::section::SectionProps> {
    let doc = &app.session.doc;
    let block = app.session.sel.focus.path.0.first().copied().unwrap_or(0) as usize;
    let i = doc.section_index_of(block);
    doc.sections().get(i).map(|(_, s)| (*s).clone())
}

// ---------------------------------------------------------------------------------------------
// Small widgets, all at the one interface size.

fn style_popup(ui: &mut Ui, app: &mut WordApp, st: &Value, width: f32) {
    let current = st.get("styleName").and_then(Value::as_str).unwrap_or("Normal").to_string();
    let styles: Vec<(String, String)> = app.session.doc.styles.gallery().iter().map(|s| (s.id.clone(), crate::i18n::style_name(s))).collect();
    let names: Vec<String> = styles.iter().map(|(_, n)| n.clone()).collect();
    if let Some(n) = mc::popup_button(ui, "margin_style", width, &current, &names, false)
        && let Some((id, _)) = styles.iter().find(|(_, name)| *name == n)
    {
        run_from_ribbon(app, "para.style", json!({"style": id}));
    }
}

fn pad(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(PAD as i8, 0)).show(ui, add);
}

// Thin wrappers over the shared controls (`mac_controls`), so the tabs above read the same.

fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    mc::section(ui, title, add);
}

/// A row label in the 80 pt label column (the control after it fills the rest).
fn label(ui: &mut Ui, text: &str, _width: f32) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(mc::LABEL_W, ROW_H), Sense::hover());
    ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, tl!(text), font(), t.text_dim);
}

fn hint(ui: &mut Ui, text: &str) {
    mc::hint(ui, text);
}

fn number(ui: &mut Ui, id: &str, value: f32, step: f32, range: std::ops::RangeInclusive<f32>, unit: &str) -> Option<f32> {
    mc::number_field(ui, id, value, step, range, unit)
}

fn check(ui: &mut Ui, text: &str, on: bool) -> bool {
    mc::checkbox(ui, text, on)
}

fn button(ui: &mut Ui, text: &str) -> bool {
    ui.allocate_ui_with_layout(vec2(ui.available_width(), ROW_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        mc::button(ui, text, false, true)
    })
    .inner
    .clicked()
}

fn segmented(ui: &mut Ui, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let items: Vec<mc::Seg> = labels.iter().map(|l| mc::Seg::Text(l)).collect();
    mc::segmented_one(ui, &labels.join("|"), &items, selected)
}

fn toggles(ui: &mut Ui, items: &[(&str, bool)]) -> Option<usize> {
    let segs: Vec<mc::Seg> = items.iter().map(|(l, _)| mc::Seg::Text(l)).collect();
    let on: Vec<bool> = items.iter().map(|(_, o)| *o).collect();
    mc::segmented(ui, "marks", &segs, &on)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn page_names_match_both_orientations_input() {
        assert_eq!(page_name(595.0, 842.0), "A4");
        assert_eq!(page_name(612.0, 792.0), "Letter");
        assert_eq!(page_name(100.0, 100.0), "Custom");
    }

    fn mac_harness() -> egui_kittest::Harness<'static, WordApp> {
        let mut a = WordApp::new(crate::Session::new(wordcraft_engine::sample::sample_document()), crate::Services::default());
        a.ui.margin = MarginPrefs::default();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        for _ in 0..6 {
            h.step();
        }
        h
    }

    /// The Mac layout replaces the ribbon; the inspector's alignment runs the command and hands
    /// focus back to the page, like the ribbon's buttons (#686).
    #[test]
    fn inspector_aligns_and_keeps_typing_focus() {
        let mut h = mac_harness();
        assert!(active(h.state()));
        assert!(h.query_by_label("Bold").is_none(), "no ribbon in the Mac layout");
        h.get_by_role_and_label(egui::accesskit::Role::RadioButton, "Center").click();
        h.step();
        h.step();
        let app = h.state();
        let rp = app.session.doc.para_at(&app.session.sel.focus).map(|p| app.session.doc.styles.resolve_para(&p.props));
        assert_eq!(rp.map(|r| r.align), Some(wordcraft_doc::Align::Center));
        assert!(app.canvas.want_focus || app.canvas.focused, "focus goes back to the page");
    }

    /// Pageless = Web Layout at the chosen text width; ⌃⌘R flips to the Classic ribbon and back.
    #[test]
    fn pageless_width_and_classic_toggle() {
        let mut h = mac_harness();
        set_pageless(h.state_mut(), true);
        for _ in 0..3 {
            h.step();
        }
        assert!(pageless(h.state()));
        assert!((h.state().session.view.web_width - 680.0).abs() < 1.0, "Medium is 680 pt");
        h.state_mut().ui.margin.text_width = TextWidth::Narrow;
        h.step();
        h.step();
        assert!((h.state().session.view.web_width - 520.0).abs() < 1.0);
        h.state_mut().ui.margin.classic_ribbon = true;
        h.step();
        h.step();
        assert!(!active(h.state()));
        assert!(h.query_by_label("Bold").is_some(), "the ribbon is back");
    }

    #[test]
    fn text_widths() {
        assert_eq!(TextWidth::Narrow.points(), Some(520.0));
        assert_eq!(TextWidth::Full.points(), None);
        assert!(MarginPrefs::default().inspector);
    }
}
