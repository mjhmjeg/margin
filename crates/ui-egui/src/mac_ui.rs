//! Margin's Mac layout: one slim toolbar in the title-bar band, a Format inspector on the right
//! (Text / Layout / Document) and the Navigation pane as the outline sidebar. The ribbon stays
//! reachable as View › Classic Ribbon (⌃⌘R). Every control runs an existing engine command.

use egui::{Color32, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
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
    /// Width of the pageless text column in points (`None`: the sheet minus 64 on each side).
    pub fn points(self) -> Option<f32> {
        match self {
            TextWidth::Narrow => Some(560.0),
            TextWidth::Medium => Some(720.0),
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
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                // R1 / E5: panel toggles, one panel open at a time. Right to left: Review, Format.
                // (The Assistant toggle waits for its panel, phase 8.)
                let review_open = app.session.view.comments_pane;
                let n = review_count(app, ui.ctx());
                if panel_toggle(ui, "review", tl!("Review"), review_open, n).on_hover_text(tl!("Comments and changes")).clicked() {
                    let open = !review_open;
                    app.session.view.comments_pane = open;
                    if open {
                        app.ui.margin.inspector = false;
                    }
                }
                let format_open = app.ui.margin.inspector;
                if panel_toggle(ui, "inspector", tl!("Format"), format_open, 0).on_hover_text(tl!("Show or hide the Format inspector")).clicked() {
                    app.ui.margin.inspector = !format_open;
                    if !format_open {
                        app.session.view.comments_pane = false;
                    }
                }
                mc::toolbar_divider(ui);
                zoom_menu(ui, app);
                mc::toolbar_divider(ui);
                // E3: comment, track changes, more.
                more_menu(ui, app);
                let tracking = app.session.doc.settings.track_changes;
                if tool(ui, app, "trackChanges", "Track changes", tracking).clicked() {
                    run_from_ribbon(app, "review.trackChanges", json!({}));
                }
                if tool(ui, app, "newComment", "Comment", false).clicked() {
                    run_from_ribbon(app, "review.newComment", json!({}));
                }
                mc::toolbar_divider(ui);
                // E2: insert group (right to left: link, shape, image, table).
                if tool(ui, app, "link", "Link", false).clicked() {
                    run_from_ribbon(app, "insert.link", json!({}));
                }
                shape_menu(ui, app);
                if tool(ui, app, "picture", "Image", false).clicked() {
                    run_from_ribbon(app, "insert.picture", json!({}));
                }
                table_menu(ui, app);
            });
        });
}

/// Pending changes plus open comments (the Review button's badge), recounted when the document changes.
fn review_count(app: &mut WordApp, ctx: &egui::Context) -> usize {
    let rev = app.session.rev();
    let id = egui::Id::new("margin_review_count");
    if let Some((r, n)) = ctx.data(|d| d.get_temp::<(u64, usize)>(id))
        && r == rev
    {
        return n;
    }
    let changes = app.session.run("review.changes", &json!({})).ok().and_then(|v| v.as_array().map(Vec::len)).unwrap_or(0);
    let comments = app.session.doc.comments.values().filter(|c| !c.resolved && c.parent.is_none()).count();
    let n = changes + comments;
    ctx.data_mut(|d| d.insert_temp(id, (rev, n)));
    n
}

/// An icon button in the toolbar; `on` draws it toggled.
fn tool(ui: &mut Ui, _app: &WordApp, icon: &str, tip: &str, on: bool) -> Response {
    mc::toolbar_button(ui, icon, tip, on)
}

/// A panel toggle at the toolbar's right end: icon + label (+ count badge); the open one is `checked`.
fn panel_toggle(ui: &mut Ui, icon: &str, label: &str, on: bool, badge: usize) -> Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font(), t.text);
    let badge_txt = if badge > 99 { "99+".to_string() } else { badge.to_string() };
    let bg = ui.painter().layout_no_wrap(badge_txt, font(), t.on_accent);
    let badge_w = if badge > 0 { (bg.size().x + 10.0).max(20.0) + 6.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(10.0 + 17.0 + 6.0 + galley.size().x + badge_w + 10.0, 28.0), Sense::click());
    let fill = if on {
        t.checked
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, mc::RADIUS, fill);
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 18.5, r.center().y), vec2(17.0, 17.0)), icon, t.icon, t.accent);
    let tx = r.min.x + 33.0;
    ui.painter().galley(pos2(tx, r.center().y - galley.size().y / 2.0), galley.clone(), t.text);
    if badge > 0 {
        let w = badge_w - 6.0;
        let b = Rect::from_min_size(pos2(tx + galley.size().x + 6.0, r.center().y - 9.0), vec2(w, 18.0));
        ui.painter().rect_filled(b, 9.0, t.accent);
        ui.painter().galley(b.center() - bg.size() / 2.0, bg, t.on_accent);
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, label));
    resp
}

fn table_menu(ui: &mut Ui, app: &mut WordApp) {
    let resp = tool(ui, app, "table", "Table", false);
    // P2: hover grid popover (dialogs::table_popover).
    egui::Popup::menu(&resp).width(200.0).show(|ui| crate::dialogs::table_popover(ui, app));
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

/// E4 / P4: the only zoom control, a popover button "100%" + chevron.
fn zoom_menu(ui: &mut Ui, app: &mut WordApp) {
    let t = Tokens::get(ui.ctx());
    let shown = if app.canvas.scale > 0.0 { app.canvas.scale / crate::canvas::PX_PER_PT } else { app.session.view.zoom };
    let pct = (shown * 100.0).round();
    let fit = app.session.view.fit.clone();
    let label = format!("{pct:.0}%");
    let galley = ui.painter().layout_no_wrap(label.clone(), font(), t.text);
    let (r, resp) = ui.allocate_exact_size(vec2(10.0 + galley.size().x + 6.0 + 12.0 + 8.0, 28.0), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let fill = if open {
        t.checked
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, mc::RADIUS, fill);
    ui.painter().galley(pos2(r.min.x + 10.0, r.center().y - galley.size().y / 2.0), galley.clone(), t.text);
    crate::icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y), vec2(13.0, 13.0)),
        "chevronDown",
        t.text_dim,
        t.text_dim,
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Zoom {label}")));
    let resp = resp.on_hover_text(tl!("Zoom"));
    egui::Popup::menu(&resp).width(200.0).show(|ui| {
        ui.set_min_width(160.0);
        for z in [50, 75, 100, 125, 150, 200] {
            if mc::menu_row(ui, &format!("{z}%"), fit.is_empty() && (pct - z as f32).abs() < 0.5).clicked() {
                run_from_ribbon(app, "view.zoom", json!({"value": z}));
            }
        }
        ui.separator();
        for (label, id, key) in [("Fit Width", "view.pageWidth", "pageWidth"), ("Fit Page", "view.onePage", "onePage")] {
            if mc::menu_row(ui, tl!(label), fit == key).clicked() {
                run_from_ribbon(app, id, json!({}));
            }
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
// Status bar (E14)

/// The 28 pt status bar: `Page 1 of 2 · 357 words · English (United States)`, then notices; on the
/// right the Pages / Pageless switch and Focus. No zoom slider (zoom lives in the toolbar).
pub fn status_bar(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("status")
        .exact_size(28.0)
        .frame(
            egui::Frame::NONE
                .fill(t.status_bar)
                .inner_margin(egui::Margin { left: 16, right: 12, top: 0, bottom: 0 })
                .stroke(Stroke::new(1.0, t.border)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                let item = |ui: &mut Ui, s: &str| -> Response {
                    ui.add(egui::Label::new(egui::RichText::new(s).font(font()).color(t.text)).sense(Sense::click()))
                };
                let dot = |ui: &mut Ui| {
                    ui.label(egui::RichText::new("·").font(font()).color(t.text_dim));
                };
                let l = app.session.layout();
                let page = l.caret_on(&app.session.sel.focus, app.session.page_hint).map(|c| c.page + 1).unwrap_or(1);
                let pages = l.pages.len().to_string();
                if item(ui, &crate::i18n::fmt(tl!("Page {page} of {pages}"), &[("page", &page.to_string()), ("pages", &pages)]))
                    .on_hover_text(tl!("Go To (⌘⌥G)"))
                    .clicked()
                {
                    let _ = app.run("ui.dialog", json!({"name": "goto"}));
                }
                dot(ui);
                let words = app.cached_word_count();
                let wtxt = if app.session.sel.is_collapsed() || crate::objects::selected(app).is_some() {
                    crate::i18n::fmt(tl!("{words} words"), &[("words", &words.to_string())])
                } else {
                    let sw = wordcraft_doc::count_words(&app.session.selected_text());
                    crate::i18n::fmt(tl!("{selected} of {words} words"), &[("selected", &sw.to_string()), ("words", &words.to_string())])
                };
                if item(ui, &wtxt).clicked() {
                    let _ = app.run("ui.dialog", json!({"name": "wordCount"}));
                }
                dot(ui);
                let lang = crate::ribbon::caret_language(app);
                let resp = item(ui, &lang).on_hover_text(tl!("Language"));
                egui::Popup::menu(&resp).show(|ui| {
                    ui.set_min_width(200.0);
                    crate::ribbon::language_menu(ui, app);
                });
                // Notices (R9's review status arrives with the Review panel, phase 7).
                if app.session.sel.focus.story != wordcraft_doc::StoryRef::Body && !crate::canvas::in_text_box(app) {
                    dot(ui);
                    item(ui, tl!("Editing header/footer"));
                }
                if app.session.doc.settings.track_changes {
                    dot(ui);
                    item(ui, tl!("Track Changes: On"));
                }
                if app.session.prefs.overtype {
                    dot(ui);
                    if item(ui, tl!("Overtype")).on_hover_text(tl!("Click to switch back to inserting")).clicked() {
                        let _ = app.run("text.overtype", json!({"value": false}));
                    }
                }
                if let Some((msg, at)) = &app.status_msg
                    && crate::now_ms() - at < 6000.0
                {
                    dot(ui);
                    ui.label(egui::RichText::new(msg).font(font()).color(t.accent_text));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                    // Focus.
                    let w = ui.painter().layout_no_wrap(tl!("Focus").to_string(), font(), t.text).size().x;
                    let (r, resp) = ui.allocate_exact_size(vec2(10.0 + 16.0 + 6.0 + w + 10.0, 22.0), Sense::click());
                    let on = app.session.view.focus_mode;
                    if on {
                        ui.painter().rect_filled(r, 5.0, t.checked);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 5.0, t.hover);
                    }
                    crate::icons::paint(
                        ui.painter(),
                        Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(16.0, 16.0)),
                        "focus",
                        t.icon,
                        t.accent,
                    );
                    ui.painter().text(pos2(r.min.x + 32.0, r.center().y), egui::Align2::LEFT_CENTER, tl!("Focus"), font(), t.text);
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, tl!("Focus")));
                    if resp.clicked() {
                        let _ = app.run("view.focus", json!({}));
                    }
                    // Pages / Pageless, icon segments 22 high.
                    let m = crate::skin::MarginTokens::get(ui.ctx());
                    let (r, _) = ui.allocate_exact_size(vec2(68.0, 22.0), Sense::hover());
                    ui.painter().rect(r, 6.0, m.segment_track, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
                    let pageless_now = pageless(app);
                    for (i, (icon, tip)) in [("viewPages", "Pages"), ("viewPageless", "Pageless")].into_iter().enumerate() {
                        let cell = Rect::from_min_size(r.min + vec2(2.0 + 32.0 * i as f32, 2.0), vec2(32.0, 18.0));
                        let resp = ui.interact(cell, ui.id().with(("status_view", i)), Sense::click());
                        let on = (i == 1) == pageless_now;
                        if on {
                            for sh in m.chip_shadow.iter().filter(|s| s.color.a() > 0) {
                                ui.painter().add(sh.as_shape(cell, 4.0));
                            }
                            ui.painter().rect_filled(cell, 4.0, m.segment_chip);
                        }
                        let ink = if on { t.text } else { t.text_dim };
                        crate::icons::paint(ui.painter(), Rect::from_center_size(cell.center(), vec2(16.0, 16.0)), icon, ink, ink);
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, tl!(tip)));
                        if resp.on_hover_text(tl!(tip)).clicked() && !on {
                            set_pageless(app, i == 1);
                        }
                    }
                });
            });
        });
}

// ---------------------------------------------------------------------------------------------
// Outline sidebar (E21-E23)

pub const OUTLINE_W: f32 = 220.0;

/// The outline sidebar: search field, Headings / Pages / Results tabs, 28 pt rows.
pub fn outline(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::MarginTokens::get(ui.ctx());
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    // E21: search field, 12 pt margin.
    let qid = egui::Id::new("nav_query");
    let mut q = ui.data(|d| d.get_temp::<String>(qid)).unwrap_or_default();
    egui::Frame::NONE.inner_margin(egui::Margin::same(12)).show(ui, |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        ui.painter().rect(r, mc::RADIUS, m.field_bg, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
        crate::icons::paint(
            ui.painter(),
            Rect::from_center_size(pos2(r.min.x + 16.0, r.center().y), vec2(15.0, 15.0)),
            "search",
            t.text_dim,
            t.text_dim,
        );
        let field = Rect::from_min_max(pos2(r.min.x + 30.0, r.min.y + 2.0), pos2(r.max.x - 8.0, r.max.y - 2.0));
        let te = ui.put(
            field,
            egui::TextEdit::singleline(&mut q)
                .frame(egui::Frame::NONE)
                .font(font())
                .hint_text(egui::RichText::new(tl!("Search document")).color(m.hint))
                .text_color(t.text),
        );
        if te.has_focus() {
            ui.painter().rect_stroke(r.expand(1.5), mc::RADIUS + 1.5, Stroke::new(3.0, m.focus_ring), egui::StrokeKind::Outside);
        }
        if te.changed() {
            ui.data_mut(|d| d.insert_temp(qid, q.clone()));
            if q.is_empty() {
                app.session.find.results.clear();
            } else {
                let _ = app.session.run("edit.find", &json!({"text": q}));
                app.ui.nav_tab = "results".into();
            }
        }
    });
    // E22: underline tabs, 40 high, hairline below.
    let tabs = [("headings", "Headings"), ("pages", "Pages"), ("results", "Results")];
    let sel = tabs.iter().position(|(k, _)| *k == app.ui.nav_tab).unwrap_or(0);
    if let Some(i) = mc::underline_tabs(ui, &tabs.map(|(_, l)| l), sel, 40.0) {
        app.ui.nav_tab = tabs.get(i).map_or("headings", |(k, _)| k).into();
    }
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.min.y, Stroke::new(1.0, t.border));
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match app.ui.nav_tab.as_str() {
            "pages" => {
                let l = app.session.layout();
                let cur = app.session.page_hint;
                for (i, p) in l.pages.iter().enumerate() {
                    let label = crate::i18n::fmt(tl!("Page {page}"), &[("page", &p.number.to_string())]);
                    if outline_row(ui, 0, &label, i == cur, None).clicked() {
                        let _ = app.run("edit.goto", json!({"page": i + 1}));
                    }
                }
            }
            "results" => egui::Frame::NONE.inner_margin(egui::Margin::same(12)).show(ui, |ui| crate::panes::nav_results(app, ui)).inner,
            _ => {
                let (items, current) = crate::panes::nav_items(app);
                if items.is_empty() {
                    egui::Frame::NONE.inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                        mc::hint(ui, "Apply Heading styles to your headings and they appear here.");
                    });
                }
                // Collapsed parents hide the headings below them.
                let mut hide_below: Option<u8> = None;
                for i in 0..items.len() {
                    let Some((path, lv, txt)) = items.get(i) else { continue };
                    if let Some(h) = hide_below {
                        if *lv > h {
                            continue;
                        }
                        hide_below = None;
                    }
                    let parent = items.get(i + 1).is_some_and(|n| n.1 > *lv);
                    let key = egui::Id::new(("outline_collapsed", path.clone()));
                    let collapsed = parent && ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
                    let chev = parent.then_some(collapsed);
                    let resp = outline_row(ui, *lv, txt, current == Some(i), chev);
                    let toggled =
                        chev.is_some() && resp.interact_pointer_pos().is_some_and(|p| p.x < resp.rect.min.x + 14.0 + 16.0 * f32::from(*lv) + 20.0);
                    if resp.clicked() && toggled {
                        ui.data_mut(|d| d.insert_temp(key, !collapsed));
                    } else if resp.clicked() {
                        let _ = app.run(
                            "caret.set",
                            json!({"pos": wordcraft_doc::Pos { story: wordcraft_doc::StoryRef::Body, path: path.clone(), off: 0 }}),
                        );
                        app.canvas.nav_to_caret = true;
                        app.canvas.scroll_to_caret = false;
                        app.canvas.want_focus = true;
                    }
                    if collapsed {
                        hide_below = Some(*lv);
                    }
                }
            }
        }
    });
}

/// E23: a 28 pt row, 14 pt indent per level, a chevron on parents, the current one in a `checked` pill.
fn outline_row(ui: &mut Ui, level: u8, text: &str, current: bool, chevron: Option<bool>) -> Response {
    let t = Tokens::get(ui.ctx());
    let (cell, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let r = cell.shrink2(vec2(8.0, 0.0));
    if current {
        ui.painter().rect_filled(r, 5.0, t.checked);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.hover);
    }
    let x = r.min.x + 8.0 + 14.0 * f32::from(level);
    if let Some(collapsed) = chevron {
        let icon = if collapsed { "chevronRight" } else { "chevronDown" };
        crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(x + 6.0, r.center().y), vec2(13.0, 13.0)), icon, t.text_dim, t.text_dim);
    }
    let tx = x + 18.0;
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font(), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width((r.max.x - tx - 8.0).max(0.0));
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(pos2(tx, r.center().y - galley.size().y / 2.0), galley, t.text);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, current, text));
    resp.on_hover_text(text)
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
            ui.spacing_mut().item_spacing = vec2(8.0, mc::ROW_GAP);
            // E6: underline tabs, 44 high, hairline below.
            let tabs = [(InspectorTab::Text, "Text"), (InspectorTab::Layout, "Layout"), (InspectorTab::Document, "Document")];
            let cur = app.ui.margin.tab;
            let sel = tabs.iter().position(|(k, _)| *k == cur).unwrap_or(0);
            if let Some(i) = mc::underline_tabs(ui, &tabs.map(|(_, l)| l), sel, 44.0) {
                app.ui.margin.tab = tabs.get(i).map_or(cur, |(k, _)| *k);
            }
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.min.y - mc::ROW_GAP, Stroke::new(1.0, t.border));
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.ui.margin.tab {
                InspectorTab::Text => text_tab(app, ui),
                InspectorTab::Layout => layout_tab(app, ui),
                InspectorTab::Document => document_tab(app, ui),
            });
        });
}

/// Font colours offered in the Colour popup (name, RRGGBB or "auto").
const COLOURS: [(&str, &str); 9] = [
    ("Automatic", "auto"),
    ("Black", "000000"),
    ("Dark grey", "595959"),
    ("Grey", "A6A6A6"),
    ("Blue", "0A64D6"),
    ("Dark blue", "1F3864"),
    ("Red", "C00000"),
    ("Green", "1F7A3A"),
    ("Orange", "B45309"),
];

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
            let f = app.previews.font_stored(&f);
            run_from_ribbon(app, "format.font", json!({"name": f}));
        }
        // E9: size (80 wide) and B I U S on one row.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let size = st.get("size").and_then(Value::as_f64).unwrap_or(12.0) as f32;
            ui.allocate_ui_with_layout(vec2(80.0, ROW_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if let Some(v) = number(ui, "margin_size", size, 1.0, 1.0..=400.0, "pt") {
                    run_from_ribbon(app, "format.size", json!({"size": v}));
                }
            });
            let marks = [
                ("bold", "Bold", "format.bold", flag("bold")),
                ("italic", "Italic", "format.italic", flag("italic")),
                ("underline", "Underline", "format.underline", flag("underline")),
                ("strike", "Strikethrough", "format.strikethrough", flag("strike")),
            ];
            let segs: Vec<mc::Seg> = marks.iter().map(|(icon, tip, _, _)| mc::Seg::Icon(icon, tip)).collect();
            let on: Vec<bool> = marks.iter().map(|m| m.3).collect();
            if let Some(i) = mc::segmented(ui, "marks", &segs, &on)
                && let Some((_, _, id, _)) = marks.get(i)
            {
                run_from_ribbon(app, id, json!({}));
            }
        });
        // E9: Colour row — swatch + popup.
        mc::labelled_row(ui, "Colour", |ui| {
            let code = st.get("color").and_then(Value::as_str).unwrap_or("auto").to_string();
            let swatch = parse_hex(&code).unwrap_or(Color32::from_gray(0x1A));
            let (sr, _) = ui.allocate_exact_size(vec2(20.0, ROW_H), Sense::hover());
            ui.painter().rect(
                Rect::from_center_size(sr.center(), vec2(18.0, 18.0)),
                4.0,
                swatch,
                Stroke::new(1.0, Tokens::get(ui.ctx()).input_border),
                egui::StrokeKind::Inside,
            );
            let name = COLOURS.iter().find(|(_, c)| c.eq_ignore_ascii_case(&code)).map_or_else(|| format!("#{code}"), |(n, _)| tl!(*n).to_string());
            let names: Vec<String> = COLOURS.iter().map(|(n, _)| tl!(*n).to_string()).collect();
            if let Some(n) = mc::popup_button(ui, "margin_colour", ui.available_width(), &name, &names, false)
                && let Some((_, c)) = COLOURS.iter().find(|(cn, _)| tl!(*cn) == n)
            {
                run_from_ribbon(app, "format.color", json!({"color": c}));
            }
        });
    });

    section(ui, "Alignment", |ui| {
        use wordcraft_doc::Align as A;
        let align = rp.as_ref().map(|r| r.align.visual(r.bidi)).unwrap_or_default();
        // E10: four icon segments, neutral chip.
        let opts = [
            ("alignLeft", "Left", A::Left, "left"),
            ("alignCenter", "Center", A::Center, "center"),
            ("alignRight", "Right", A::Right, "right"),
            ("justify", "Justify", A::Justify, "justify"),
        ];
        let segs: Vec<mc::Seg> = opts.iter().map(|(icon, tip, _, _)| mc::Seg::Icon(icon, tip)).collect();
        if let Some(i) = mc::segmented_one(ui, "align", &segs, opts.iter().position(|o| o.2 == align))
            && let Some((_, _, _, v)) = opts.get(i)
        {
            run_from_ribbon(app, "para.align", json!({"value": v}));
        }
    });

    section(ui, "Spacing", |ui| {
        let (before, after, line) = rp.as_ref().map_or((0.0, 0.0, 1.0), |r| (r.space_before, r.space_after, line_multiple(&r.line_spacing)));
        if let Some(v) = mc::labelled_row(ui, "Line", |ui| number(ui, "margin_line", line, 0.05, 0.5..=5.0, "×")) {
            run_from_ribbon(app, "para.lineSpacing", json!({"value": v}));
        }
        if let Some(v) = mc::labelled_row(ui, "Before", |ui| number(ui, "margin_before", before, 1.0, 0.0..=400.0, "pt")) {
            run_from_ribbon(app, "para.spacing", json!({"before": v}));
        }
        if let Some(v) = mc::labelled_row(ui, "After", |ui| number(ui, "margin_after", after, 1.0, 0.0..=400.0, "pt")) {
            run_from_ribbon(app, "para.spacing", json!({"after": v}));
        }
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

    // E12: the full dialogs, as links.
    egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 16, top: 16, bottom: 16 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 20.0;
            if mc::link(ui, "Font…").clicked() {
                run_from_ribbon(app, "format.fontDialog", json!({}));
            }
            if mc::link(ui, "Paragraph options…").clicked() {
                run_from_ribbon(app, "para.dialog", json!({}));
            }
        });
    });
}

fn parse_hex(code: &str) -> Option<Color32> {
    let c = code.trim_start_matches('#');
    if c.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(c, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn layout_tab(app: &mut WordApp, ui: &mut Ui) {
    let rp = app.session.doc.para_at(&app.session.sel.focus).map(|p| app.session.doc.styles.resolve_para(&p.props));
    let (left, right, first) = rp.as_ref().map_or((0.0, 0.0, 0.0), |r| (r.indent_left, r.indent_right, r.indent_first));
    section(ui, "Indents", |ui| {
        for (name, key, v) in [("Left", "left", left), ("Right", "right", right), ("First line", "firstLine", first)] {
            if let Some(cm) = mc::labelled_row(ui, name, |ui| number(ui, &format!("margin_ind_{key}"), v / CM, 0.1, -20.0..=50.0, "cm")) {
                run_from_ribbon(app, "para.indents", json!({ key: cm * CM }));
            }
        }
    });
    section(ui, "Columns", |ui| {
        let sect = caret_section(app);
        let count = sect.as_ref().map_or(1, |s| s.columns.count as usize).clamp(1, 3);
        if let Some(i) = segmented(ui, &["1", "2", "3"], Some(count - 1)) {
            run_from_ribbon(app, "layout.columns", json!({"count": i + 1}));
        }
        // E15: gap between columns, disabled at one column.
        let gap = sect.as_ref().map_or(36.0, |s| s.columns.space) / CM;
        ui.add_enabled_ui(count > 1, |ui| {
            if let Some(cm) = mc::labelled_row(ui, "Gap", |ui| number(ui, "margin_col_gap", gap, 0.05, 0.0..=10.0, "cm")) {
                run_from_ribbon(app, "layout.columns", json!({"count": count, "space": cm * CM}));
            }
        });
    });
    section(ui, "Pagination", |ui| {
        // E16.
        let (keep_next, keep_lines, break_before, widow) =
            rp.as_ref().map_or((false, false, false, true), |r| (r.keep_next, r.keep_lines, r.page_break_before, r.widow_control));
        for (label, on, id) in [
            ("Keep with next", keep_next, "para.keepNext"),
            ("Keep lines together", keep_lines, "para.keepLines"),
            ("Page break before", break_before, "para.pageBreakBefore"),
            ("Widow and orphan control", widow, "para.widowControl"),
        ] {
            if check(ui, label, on) {
                run_from_ribbon(app, id, json!({}));
            }
        }
    });
}

fn document_tab(app: &mut WordApp, ui: &mut Ui) {
    let pageless = pageless(app);
    section(ui, "Format", |ui| {
        if let Some(i) = segmented(ui, &["Pages", "Pageless"], Some(usize::from(pageless))) {
            set_pageless(app, i == 1);
        }
        hint(ui, "Pageless hides page breaks, headers and footers. They come back when you print or export to PDF.");
    });
    if pageless {
        // E20: Text width instead of Page and Margins.
        section(ui, "Text width", |ui| {
            let opts = [(TextWidth::Narrow, "Narrow"), (TextWidth::Medium, "Medium"), (TextWidth::Full, "Full")];
            let cur = app.ui.margin.text_width;
            if let Some(i) = segmented(ui, &opts.map(|(_, l)| l), opts.iter().position(|(w, _)| *w == cur)) {
                app.ui.margin.text_width = opts.get(i).map_or(cur, |(w, _)| *w);
            }
        });
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
        // E18: 2 × 2 grid, label above field.
        let rows = [
            [("Top", "top", sect.margin_top), ("Bottom", "bottom", sect.margin_bottom)],
            [("Left", "left", sect.margin_left), ("Right", "right", sect.margin_right)],
        ];
        let t = Tokens::get(ui.ctx());
        for row in rows {
            ui.columns(2, |cols| {
                for (col, (name, key, v)) in cols.iter_mut().zip(row) {
                    col.spacing_mut().item_spacing.y = 4.0;
                    col.label(egui::RichText::new(tl!(name)).font(font()).color(t.text_dim));
                    if let Some(cm) = number(col, &format!("margin_m_{key}"), v / CM, 0.1, 0.0..=20.0, "cm") {
                        run_from_ribbon(app, "layout.margins", json!({ key: cm * CM }));
                    }
                }
            });
        }
    });
    section(ui, "Headers and footers", |ui| {
        // E19: switches; "Edit" on rows that are on.
        let header = sect.headers.default.is_some();
        let footer = sect.footers.default.is_some();
        let numbers = has_page_numbers(app, &sect);
        for (label, on, add, edit, remove) in [
            ("Header", header, ("insert.header", json!({"preset": "blank"})), "insert.editHeader", "insert.removeHeader"),
            ("Footer", footer, ("insert.footer", json!({"preset": "blank"})), "insert.editFooter", "insert.removeFooter"),
        ] {
            let top = ui.cursor().min;
            let flip = mc::switch(ui, label, on);
            if on {
                let lw = ui.painter().layout_no_wrap(tl!(label).to_string(), font(), Color32::WHITE).size().x;
                let rect = Rect::from_min_size(pos2(top.x + lw + 12.0, top.y), vec2(40.0, ROW_H));
                let clicked = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                        mc::link(ui, "Edit").clicked()
                    })
                    .inner;
                if clicked {
                    run_from_ribbon(app, edit, json!({}));
                }
            }
            if flip {
                if on {
                    run_from_ribbon(app, remove, json!({}));
                } else {
                    run_from_ribbon(app, add.0, add.1);
                }
            }
        }
        if mc::switch(ui, "Page numbers", numbers) {
            if numbers {
                // No engine command removes just the number yet (it is a field in the footer).
                app.status(tl!("Page numbers sit in the footer: choose Edit on Footer to remove them."));
            } else {
                run_from_ribbon(app, "insert.pageNumber", json!({"position": "bottom", "align": "center"}));
            }
        }
        if check(ui, "Different first page", sect.title_page) {
            run_from_ribbon(app, "layout.differentFirstPage", json!({}));
        }
    });
}

/// Whether the section's header or footer holds a PAGE field.
fn has_page_numbers(app: &WordApp, sect: &wordcraft_doc::section::SectionProps) -> bool {
    let doc = &app.session.doc;
    [sect.headers.default, sect.footers.default, sect.headers.first, sect.footers.first].into_iter().flatten().any(|id| {
        let story = wordcraft_doc::StoryRef::Part(id);
        doc.para_paths(story).iter().filter_map(|p| doc.para(story, p)).any(|para| {
            para.objects
                .iter()
                .any(|o| matches!(o, wordcraft_doc::InlineObject::Field { instr, .. } if instr.trim().to_ascii_uppercase().starts_with("PAGE")))
        })
    })
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
    let full = (avail_w / px_per_pt - 128.0).max(200.0);
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

// Thin wrappers over the shared controls (`mac_controls`), so the tabs above read the same.

fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    mc::section(ui, title, add);
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

fn segmented(ui: &mut Ui, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let items: Vec<mc::Seg> = labels.iter().map(|l| mc::Seg::Text(l)).collect();
    mc::segmented_one(ui, &labels.join("|"), &items, selected)
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
        assert!(h.query_by_role_and_label(egui::accesskit::Role::Button, "Bold").is_none(), "no ribbon in the Mac layout");
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
        assert!((h.state().session.view.web_width - 720.0).abs() < 1.0, "Medium is 720 pt");
        h.state_mut().ui.margin.text_width = TextWidth::Narrow;
        h.step();
        h.step();
        assert!((h.state().session.view.web_width - 560.0).abs() < 1.0);
        h.state_mut().ui.margin.classic_ribbon = true;
        h.step();
        h.step();
        assert!(!active(h.state()));
        assert!(h.query_by_label("Bold").is_some(), "the ribbon is back");
    }

    /// E14 / P4: the status bar's Pages / Pageless switch works, there is one zoom control (the
    /// toolbar's popover) and it runs `view.zoom`.
    #[test]
    fn status_switch_and_zoom_popover() {
        use egui::accesskit::Role;
        let mut h = mac_harness();
        h.get_by_role_and_label(Role::RadioButton, "Pageless").click();
        h.step();
        h.step();
        assert!(pageless(h.state()));
        h.get_by_role_and_label(Role::RadioButton, "Pages").click();
        h.step();
        h.step();
        assert!(!pageless(h.state()));
        assert!(h.query_by_label("Zoom In").is_none(), "no second zoom control in the status bar");
        h.get_by_label("Zoom 100%").click();
        h.step();
        h.step();
        h.get_by_label("50%").click();
        h.step();
        h.step();
        assert!((h.state().session.view.zoom - 0.5).abs() < 1e-3);
    }

    #[test]
    fn text_widths() {
        assert_eq!(TextWidth::Narrow.points(), Some(560.0));
        assert_eq!(TextWidth::Full.points(), None);
        assert!(MarginPrefs::default().inspector);
    }
}
