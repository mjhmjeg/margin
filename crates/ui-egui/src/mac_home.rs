//! Margin's Home screen, Pages-style: a full-height sidebar (New, Recent, Open…, the document
//! group, Settings) and a document chooser — template cards or recent documents as page
//! thumbnails (grid or list), with Open… / Cancel / Create at the bottom. Shown at launch and on
//! File › New. The document pages (Save As, Print, Export, Options) are upstream's, in the same frame.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::WordApp;
use crate::mac_controls::{self as mc, Seg, font};
use crate::theme::Tokens;

const SIDEBAR_W: f32 = 220.0;
/// Traffic lights sit in the sidebar's top band.
const LIGHTS_BAND: f32 = 36.0;
const CARD_W: f32 = 168.0;
const CARD_H: f32 = (CARD_W - 2.0 * RING) * 11.0 / 8.5;
const COL_GAP: f32 = 36.0;
const ROW_GAP: f32 = 32.0;
const INSET: i8 = 40;
const HEADER_H: f32 = 52.0;
const FOOTER_H: f32 = 60.0;
const LIST_ROW_H: f32 = 44.0;
const LIST_THUMB_H: f32 = 32.0;
/// Paper corners: barely rounded, like a sheet, not a tile.
const PAPER_RADIUS: f32 = 3.0;
/// Selection ring: a gap from the paper, then the ring.
const RING_GAP: f32 = 3.0;
const RING_W: f32 = 2.5;
const RING: f32 = RING_GAP + RING_W;
/// Recent files bigger than this show an icon instead of a rendered first page.
const MAX_THUMB_BYTES: u64 = 20 << 20;

/// Sidebar entries: page id, label, icon.
const MAIN: [(&str, &str, &str); 3] = [("new", "New", "newDocument"), ("recent", "Recent", "recent"), ("open", "Open…", "open")];
const DOC: [(&str, &str, &str); 3] = [("saveAs", "Save As…", "saveAs"), ("print", "Print…", "print"), ("export", "Export…", "share")];

/// What the chooser has selected, kept between frames.
#[derive(Clone, Default)]
struct Pick(Option<String>);

/// "A document is open": it has a file or unsaved edits. Home over an untouched blank window
/// (launch) has nothing to save, print or cancel back to.
fn document_open(app: &WordApp) -> bool {
    app.session.path.is_some() || app.session.dirty
}

/// The Home screen in place of upstream's Backstage.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // Upstream's start page is "home"; Margin's is the template chooser.
    if app.ui.backstage_page == "home" {
        app.ui.backstage_page = "new".into();
    }
    sidebar(app, ui, &t);
    let page = app.ui.backstage_page.clone();
    match page.as_str() {
        "new" | "recent" => chooser(app, ui, &t, page == "recent"),
        _ => {
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.ribbon).inner_margin(egui::Margin::symmetric(40, 30))).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match page.as_str() {
                    "saveAs" => crate::backstage::save_as_page(app, ui),
                    "info" => crate::backstage::info_page(app, ui),
                    "print" | "export" => crate::backstage::export_page(app, ui),
                    "options" => crate::options::page(app, ui),
                    _ => {}
                });
            });
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !app.file_dialog_open() {
        close(app);
    }
}

fn close(app: &mut WordApp) {
    app.ui.backstage = false;
    app.canvas.want_focus = true;
}

fn sidebar(app: &mut WordApp, ui: &mut Ui, t: &Tokens) {
    let mac = app.integrated_titlebar;
    let doc = document_open(app);
    egui::Panel::left("margin_home_nav")
        .exact_size(SIDEBAR_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 0,
            bottom: 12,
        }))
        .show(ui, |ui| {
            // The traffic lights' band drags the window.
            let (_, drag) = ui.allocate_exact_size(vec2(SIDEBAR_W - 20.0, if mac { LIGHTS_BAND } else { 12.0 }), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            ui.spacing_mut().item_spacing.y = 2.0;
            for (id, label, icon) in MAIN {
                nav_item(app, ui, t, id, label, icon, true);
            }
            divider(ui, t);
            group_label(ui, t, "Document");
            for (id, label, icon) in DOC {
                nav_item(app, ui, t, id, label, icon, doc);
            }
            // Settings is pinned to the bottom.
            let rest = ui.available_height() - 30.0;
            ui.add_space(rest.max(0.0));
            nav_item(app, ui, t, "options", "Settings", "settings", true);
        });
}

fn divider(ui: &mut Ui, t: &Tokens) {
    ui.add_space(8.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range().shrink(8.0), r.min.y, Stroke::new(1.0, t.border));
    ui.add_space(10.0);
}

fn group_label(ui: &mut Ui, t: &Tokens, label: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, tl!(label), font(), t.text_dim);
}

fn nav_item(app: &mut WordApp, ui: &mut Ui, t: &Tokens, id: &str, label: &str, icon: &str, enabled: bool) {
    if row(ui, t, label, icon, app.ui.backstage_page == id, enabled).clicked() && enabled {
        match id {
            "open" => {
                let _ = app.run("ui.openFileDialog", json!({}));
            }
            _ => app.ui.backstage_page = id.into(),
        }
    }
}

/// A sidebar row: 30 high, 16 pt icon + 10 gap; a `checked` pill when selected, `hover` on hover.
/// A disabled row is `text_dim` and inert.
fn row(ui: &mut Ui, t: &Tokens, label: &str, icon: &str, selected: bool, enabled: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), if enabled { Sense::click() } else { Sense::hover() });
    if selected && enabled {
        ui.painter().rect_filled(r, 6.0, t.checked);
    } else if resp.hovered() && enabled {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    let ink = if enabled { t.text } else { t.text_dim };
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 20.0, r.center().y), vec2(16.0, 16.0)), icon, ink, ink);
    ui.painter().text(pos2(r.min.x + 12.0 + 16.0 + 10.0, r.center().y), Align2::LEFT_CENTER, tl!(label), font(), ink);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, tl!(label)));
    resp
}

/// Recent order: by last change (newest first) or by name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Date,
    Name,
}

/// Templates or recent documents, with a header (title, search), a footer and the grid or list.
fn chooser(app: &mut WordApp, ui: &mut Ui, t: &Tokens, recent: bool) {
    let pick_id = egui::Id::new(("margin_home_pick", recent));
    let query_id = egui::Id::new("margin_home_query");
    let sort_id = egui::Id::new("margin_home_sort");
    let list_id = egui::Id::new("margin_home_list");
    let mut pick = ui.data(|d| d.get_temp::<Pick>(pick_id)).unwrap_or_default().0;
    let mut query = ui.data(|d| d.get_temp::<String>(query_id)).unwrap_or_default();
    let mut sort = ui.data(|d| d.get_temp::<bool>(sort_id)).map_or(Sort::Date, |n| if n { Sort::Name } else { Sort::Date });
    let mut list = recent && ui.data(|d| d.get_temp::<bool>(list_id)).unwrap_or(false);
    let mut open: Option<String> = None;
    let doc = document_open(app);

    egui::Panel::top("margin_home_top")
        .exact_size(HEADER_H)
        .frame(egui::Frame::NONE.fill(t.title_bar).inner_margin(egui::Margin::symmetric(INSET, 0)))
        .show(ui, |ui| {
            let r = ui.max_rect();
            ui.painter().hline(r.x_range().expand(INSET as f32), r.max.y, Stroke::new(1.0, t.border));
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(tl!(if recent { "Recent documents" } else { "Choose a template" })).font(font()).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    if search_field(ui, t, &mut query, if recent { "Search recent" } else { "Search templates" }) {
                        ui.data_mut(|d| d.insert_temp(query_id, query.clone()));
                    }
                    if recent {
                        let (seg, _) = ui.allocate_exact_size(vec2(72.0, mc::ROW_H), Sense::hover());
                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(seg));
                        let items = [Seg::Icon("viewGrid", "Grid"), Seg::Icon("viewList", "List")];
                        if let Some(i) = mc::segmented_one(&mut child, "home_view", &items, Some(usize::from(list))) {
                            list = i == 1;
                            ui.data_mut(|d| d.insert_temp(list_id, list));
                        }
                        let names = ["Date".to_string(), "Name".to_string()];
                        if let Some(c) = mc::popup_button(ui, "home_sort", 96.0, if sort == Sort::Name { "Name" } else { "Date" }, &names, false) {
                            sort = if c == "Name" { Sort::Name } else { Sort::Date };
                            ui.data_mut(|d| d.insert_temp(sort_id, sort == Sort::Name));
                        }
                        ui.label(egui::RichText::new(tl!("Sort")).font(font()).color(t.text_dim));
                    }
                });
            });
        });
    egui::Panel::bottom("margin_home_footer")
        .exact_size(FOOTER_H)
        .frame(egui::Frame::NONE.fill(t.title_bar).inner_margin(egui::Margin::symmetric(INSET, 0)))
        .show(ui, |ui| {
            let r = ui.max_rect();
            ui.painter().hline(r.x_range().expand(INSET as f32), r.min.y, Stroke::new(1.0, t.border));
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                if mc::button(ui, "Open…", false, true).clicked() {
                    let _ = app.run("ui.openFileDialog", json!({}));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    if mc::button(ui, if recent { "Open" } else { "Create" }, true, pick.is_some()).clicked() {
                        open = pick.clone();
                    }
                    if !recent && doc && mc::button(ui, "Cancel", false, true).clicked() {
                        close(app);
                    }
                });
            });
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: INSET, right: INSET, top: 32, bottom: 24 }))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let items = chooser_items(app, recent, &query, sort);
                if items.is_empty() {
                    if recent && query.trim().is_empty() {
                        ui.add_space(60.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(tl!("Documents you open will appear here.")).font(font()).color(t.text_dim));
                            ui.add_space(12.0);
                            if mc::button(ui, "Open…", false, true).clicked() {
                                let _ = app.run("ui.openFileDialog", json!({}));
                            }
                        });
                    } else {
                        let msg = if recent { "No recent documents match your search." } else { "No templates match your search." };
                        ui.label(egui::RichText::new(tl!(msg)).font(font()).color(t.text_dim));
                    }
                    return;
                }
                if !recent && pick.is_none() {
                    pick = items.first().map(|(k, _, _)| k.clone());
                }
                if list {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (key, label, sub) in &items {
                        let selected = pick.as_deref() == Some(key.as_str());
                        let resp = list_row(app, ui, t, key, label, sub.as_deref().unwrap_or(""), selected);
                        if resp.clicked() {
                            pick = Some(key.clone());
                        }
                        if resp.double_clicked() {
                            open = Some(key.clone());
                        }
                    }
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(COL_GAP - 2.0 * RING, ROW_GAP - 2.0 * RING);
                    for (key, label, sub) in &items {
                        let selected = pick.as_deref() == Some(key.as_str());
                        let resp = card(app, ui, t, key, label, sub.as_deref(), selected, recent);
                        if resp.clicked() {
                            pick = Some(key.clone());
                        }
                        if resp.double_clicked() {
                            open = Some(key.clone());
                        }
                    }
                });
            });
        });
    ui.data_mut(|d| d.insert_temp(pick_id, Pick(pick)));
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ui.ctx().egui_wants_keyboard_input() {
        open = ui.data(|d| d.get_temp::<Pick>(pick_id)).and_then(|p| p.0);
    }
    if let Some(key) = open {
        let r = if recent { app.run("file.open", json!({"path": key})) } else { app.run("file.new", json!({"template": key})) };
        if r.is_ok() {
            close(app);
        }
    }
}

/// Header search field: 240 × 28, search icon on the left; returns true when the text changed.
fn search_field(ui: &mut Ui, t: &Tokens, query: &mut String, hint: &str) -> bool {
    let m = crate::skin::MarginTokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(240.0, mc::ROW_H), Sense::hover());
    ui.painter().rect(r, mc::RADIUS, m.field_bg, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 15.0, r.center().y), vec2(15.0, 15.0)), "search", t.text_dim, t.text_dim);
    let inner = Rect::from_min_max(pos2(r.min.x + 28.0, r.min.y), pos2(r.max.x - 8.0, r.max.y));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child
        .add(
            egui::TextEdit::singleline(query)
                .frame(egui::Frame::NONE)
                .hint_text(tl!(hint))
                .font(font())
                .text_color(t.text)
                .desired_width(inner.width())
                .vertical_align(egui::Align::Center),
        )
        .changed()
}

/// (key, caption, "folder · date") for what the chooser shows, filtered by the search text.
fn chooser_items(app: &WordApp, recent: bool, query: &str, sort: Sort) -> Vec<(String, String, Option<String>)> {
    if !recent {
        return crate::backstage::matching_templates(query).into_iter().map(|(label, id)| (id.to_string(), tl!(label).to_string(), None)).collect();
    }
    let paths: Vec<String> = app.ui.recent.iter().take(24).cloned().collect();
    let names = display_names(&paths);
    let q = query.trim().to_lowercase();
    let mut items: Vec<(i64, String, String, String)> = paths
        .into_iter()
        .zip(names)
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .map(|(p, n)| (modified_secs(&p), p.clone(), n, folder_and_date(&p)))
        .collect();
    match sort {
        // The recent list is already newest-opened first; ties keep that order.
        Sort::Date => items.sort_by_key(|i| std::cmp::Reverse(i.0)),
        Sort::Name => items.sort_by_key(|i| i.2.to_lowercase()),
    }
    items.into_iter().map(|(_, p, n, s)| (p, n, Some(s))).collect()
}

/// Captions for recent files: the name without its extension, unless two files would then read
/// the same ("Report.docx" and "Report.odt"), which keep the extension.
fn display_names(paths: &[String]) -> Vec<String> {
    let stem = |p: &str| std::path::Path::new(p).file_stem().map_or_else(|| p.to_string(), |n| n.to_string_lossy().to_string());
    let stems: Vec<String> = paths.iter().map(|p| stem(p).to_lowercase()).collect();
    paths.iter().zip(&stems).map(|(p, s)| if stems.iter().filter(|o| *o == s).count() > 1 { file_name(p) } else { stem(p) }).collect()
}

fn modified_secs(p: &str) -> i64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64)
}

/// A recent document as a list row: 32 high thumbnail, name, then "folder · date" in `text_dim`.
fn list_row(app: &mut WordApp, ui: &mut Ui, t: &Tokens, key: &str, label: &str, sub: &str, selected: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
    if selected {
        ui.painter().rect_filled(r, 6.0, t.checked);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    let th = Rect::from_min_size(pos2(r.min.x + 12.0, r.center().y - LIST_THUMB_H / 2.0), vec2(LIST_THUMB_H * 8.5 / 11.0, LIST_THUMB_H));
    ui.painter().rect_filled(th, 2.0, Color32::WHITE);
    match recent_thumb(app, ui, key) {
        Some(h) => {
            ui.painter().add(
                egui::epaint::RectShape::filled(th, 2.0, Color32::WHITE).with_texture(h.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
            );
        }
        None => crate::icons::paint(ui.painter(), th.shrink(4.0), "printLayout", t.text_disabled, t.text_disabled),
    }
    ui.painter().rect_stroke(th, 2.0, Stroke::new(0.5, Color32::from_black_alpha(30)), egui::StrokeKind::Outside);
    let x = th.max.x + 14.0;
    let w = (r.max.x - x - 12.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), font(), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(w * 0.5);
    let g = ui.painter().layout_job(job);
    ui.painter().galley(pos2(x, r.center().y - g.size().y / 2.0), g, t.text);
    ui.painter().text(pos2(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, sub, font(), t.text_dim);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label));
    resp
}

/// One page-shaped card with its label (and folder/date for recent documents) underneath.
#[allow(clippy::too_many_arguments)]
fn card(app: &mut WordApp, ui: &mut Ui, t: &Tokens, key: &str, label: &str, sub: Option<&str>, selected: bool, recent: bool) -> egui::Response {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.set_width(CARD_W);
        // Room around the page for the selection ring (2 pt gap + 2.5 pt ring).
        let (outer, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H + 2.0 * RING), Sense::click());
        let r = Rect::from_center_size(outer.center(), vec2(CARD_W - 2.0 * RING, CARD_H));
        // Paper: barely rounded, with a soft shadow that lifts on hover.
        let shadow = egui::epaint::Shadow {
            offset: [0, if resp.hovered() { 4 } else { 1 }],
            blur: if resp.hovered() { 14 } else { 6 },
            spread: 0,
            color: Color32::from_black_alpha(if resp.hovered() { 46 } else { 30 }),
        };
        ui.painter().add(shadow.as_shape(r, PAPER_RADIUS));
        ui.painter().rect_filled(r, PAPER_RADIUS, Color32::WHITE);
        let tex = if recent { recent_thumb(app, ui, key) } else { template_thumb(app, ui, key) };
        match tex {
            Some(h) => {
                ui.painter().add(
                    egui::epaint::RectShape::filled(r, PAPER_RADIUS, Color32::WHITE)
                        .with_texture(h.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
                );
            }
            None => {
                crate::icons::paint(
                    ui.painter(),
                    Rect::from_center_size(r.center(), vec2(40.0, 40.0)),
                    "printLayout",
                    t.text_disabled,
                    t.text_disabled,
                );
            }
        }
        ui.painter().rect_stroke(r, PAPER_RADIUS, Stroke::new(0.5, Color32::from_black_alpha(30)), egui::StrokeKind::Outside);
        if selected {
            ui.painter().rect_stroke(r.expand(RING_GAP), PAPER_RADIUS + RING_GAP, Stroke::new(RING_W, t.accent), egui::StrokeKind::Outside);
        }
        ui.add_space(6.0);
        // Up to two centred lines, then an ellipsis.
        let mut job = egui::text::LayoutJob::single_section(label.to_string(), egui::text::TextFormat::simple(font(), t.text));
        job.wrap = egui::text::TextWrapping {
            max_width: CARD_W,
            max_rows: if recent { 1 } else { 2 },
            break_anywhere: false,
            overflow_character: Some('…'),
        };
        job.halign = egui::Align::Center;
        ui.vertical_centered(|ui| {
            ui.label(job).on_hover_text(label);
            if let Some(s) = sub {
                ui.label(egui::RichText::new(s).font(font()).color(t.text_dim));
            }
        });
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label));
        resp
    })
    .inner
}

/// A template's first page, rendered once per scale (cached in egui's memory).
fn template_thumb(app: &WordApp, ui: &Ui, template: &str) -> Option<egui::TextureHandle> {
    let ppp = ui.ctx().pixels_per_point();
    let key = format!("margin_tpl:{template}:{ppp}");
    if let Some(h) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(egui::Id::new(&key))) {
        return Some(h);
    }
    let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
    s.system_region = app.session.region();
    let _ = s.run("file.new", &json!({"template": template}));
    first_page_texture(ui, &mut s, &key)
}

/// A recent document's first page: rendered once per file version, at most one new file per frame
/// so a long Recent list doesn't freeze the window. Missing, unreadable or very large files get
/// the plain icon.
fn recent_thumb(_app: &WordApp, ui: &Ui, path: &str) -> Option<egui::TextureHandle> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_THUMB_BYTES {
        return None;
    }
    let stamp = meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    let ppp = ui.ctx().pixels_per_point();
    let key = format!("margin_recent:{path}:{stamp}:{ppp}");
    let id = egui::Id::new(&key);
    if let Some(h) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(h);
    }
    if ui.ctx().data(|d| d.get_temp::<bool>(id.with("failed"))).unwrap_or(false) {
        return None;
    }
    // ponytail: one render per frame on the UI thread; move to a worker if big documents stutter.
    let frame = ui.ctx().cumulative_frame_nr();
    let budget = egui::Id::new("margin_recent_budget");
    if ui.ctx().data(|d| d.get_temp::<u64>(budget)) == Some(frame) {
        ui.ctx().request_repaint();
        return None;
    }
    ui.ctx().data_mut(|d| d.insert_temp(budget, frame));
    let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
    let tex = s.run("file.open", &json!({"path": path})).ok().and_then(|_| first_page_texture(ui, &mut s, &key));
    if tex.is_none() {
        ui.ctx().data_mut(|d| d.insert_temp(id.with("failed"), true));
    }
    tex
}

fn first_page_texture(ui: &Ui, s: &mut wordcraft_engine::Session, key: &str) -> Option<egui::TextureHandle> {
    let ppp = ui.ctx().pixels_per_point();
    let l = s.export_layout();
    let page = l.pages.first()?;
    let img = wordcraft_render::render_page(&s.doc, page, (CARD_W - 2.0 * RING) / page.w * ppp, &crate::canvas::screen_render_options());
    let px = img.to_straight();
    let h = ui.ctx().load_texture(
        key,
        egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &px),
        egui::TextureOptions::LINEAR,
    );
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(key), h.clone()));
    Some(h)
}

fn file_name(p: &str) -> String {
    std::path::Path::new(p).file_name().map_or_else(|| p.to_string(), |n| n.to_string_lossy().to_string())
}

/// "Desktop · 12 Oct" for a recent file: its folder and last change.
fn folder_and_date(p: &str) -> String {
    let path = std::path::Path::new(p);
    let folder = path.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let date = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| short_date(d.as_secs() as i64))
        .unwrap_or_default();
    if date.is_empty() { folder } else { format!("{folder} · {date}") }
}

/// Day and month from Unix seconds (UTC; good enough for a card caption).
fn short_date(secs: i64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    // Civil-from-days (Howard Hinnant).
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    MONTHS.get((month - 1) as usize).map_or_else(String::new, |m| format!("{day} {m}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captions_drop_extensions_unless_names_clash() {
        let p = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(display_names(&p(&["/a/Report.docx", "/a/Notes.odt"])), ["Report", "Notes"]);
        assert_eq!(display_names(&p(&["/a/Report.docx", "/b/report.odt", "/a/x.docx"])), ["Report.docx", "report.odt", "x"]);
    }

    #[test]
    fn dates() {
        assert_eq!(short_date(0), "1 Jan");
        assert_eq!(short_date(1_791_849_600), "13 Oct"); // 2026-10-13
        assert_eq!(folder_and_date("/nowhere/Folder/x.docx"), "Folder");
    }

    /// Launch lands on the template chooser; Create opens the picked template and closes Home.
    #[test]
    fn create_from_the_chooser() {
        let mut a = WordApp::new(crate::Session::new(wordcraft_doc::Document::new()), crate::Services::default());
        a.ui.start_screen = true;
        assert!(a.start_screen());
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
        assert_eq!(h.state().ui.backstage_page, "new");
        use egui_kittest::kittest::Queryable;
        h.get_by_role_and_label(egui::accesskit::Role::Button, tl!("Letter")).click();
        h.step();
        h.get_by_role_and_label(egui::accesskit::Role::Button, tl!("Create")).click();
        for _ in 0..3 {
            h.step();
        }
        assert!(!h.state().ui.backstage, "Home closed");
        assert!(h.state().session.doc.plain_text(wordcraft_doc::StoryRef::Body).contains("Your Name"), "the Letter template opened");
    }
}
