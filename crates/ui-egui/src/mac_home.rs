//! Margin's Home screen, Pages-style: a light sidebar (New, Recent, Open…, Save As, Print, Export,
//! More…) and a document chooser — large template cards or recent documents as page thumbnails,
//! with Cancel / Create at the bottom. Shown at launch and on File › New. The document pages
//! (Save As, Print, Export, Info, Options) are upstream's, shown in the same frame.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::WordApp;
use crate::theme::{Tokens, regular};

const SIDEBAR_W: f32 = 220.0;
const CARD_W: f32 = 168.0;
const CARD_H: f32 = CARD_W * 11.0 / 8.5;
/// Recent files bigger than this show an icon instead of a rendered first page.
const MAX_THUMB_BYTES: u64 = 20 << 20;

fn font() -> egui::FontId {
    regular(crate::skin::UI_SIZE)
}

/// Sidebar entries: page id and label. `More…` reveals the ones after it.
const MAIN: [(&str, &str); 3] = [("new", "New"), ("recent", "Recent"), ("open", "Open…")];
const DOC: [(&str, &str); 3] = [("saveAs", "Save As"), ("print", "Print"), ("export", "Export")];
const MORE: [(&str, &str); 3] = [("info", "Info"), ("save", "Save"), ("options", "Options")];

/// What the chooser has selected, kept between frames.
#[derive(Clone, Default)]
struct Pick(Option<String>);

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
            let (_, drag) =
                ui.allocate_exact_size(vec2(SIDEBAR_W - 20.0, if mac { crate::chrome::MAC_TITLE_BAR + 12.0 } else { 12.0 }), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            ui.spacing_mut().item_spacing.y = 2.0;
            for (id, label) in MAIN {
                nav_item(app, ui, t, id, label);
            }
            divider(ui, t);
            for (id, label) in DOC {
                nav_item(app, ui, t, id, label);
            }
            let more_id = egui::Id::new("margin_home_more");
            let mut more = ui.data(|d| d.get_temp::<bool>(more_id)).unwrap_or(false) || MORE.iter().any(|(id, _)| app.ui.backstage_page == *id);
            if row(ui, t, if more { "Less" } else { "More…" }, false, t.text_dim).clicked() {
                more = !more;
                ui.data_mut(|d| d.insert_temp(more_id, more));
            }
            if more {
                for (id, label) in MORE {
                    nav_item(app, ui, t, id, label);
                }
            }
        });
}

fn divider(ui: &mut Ui, t: &Tokens) {
    ui.add_space(8.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range().shrink(8.0), r.min.y, Stroke::new(1.0, t.border));
    ui.add_space(10.0);
}

fn nav_item(app: &mut WordApp, ui: &mut Ui, t: &Tokens, id: &str, label: &str) {
    if row(ui, t, label, app.ui.backstage_page == id, t.text).clicked() {
        match id {
            "open" => {
                let _ = app.run("ui.openFileDialog", json!({}));
            }
            "save" => {
                let saved = app.run("file.save", json!({})).is_ok_and(|v| v.get("saved").and_then(serde_json::Value::as_bool) == Some(true));
                if saved {
                    close(app);
                } else {
                    app.ui.backstage_page = "saveAs".into();
                }
            }
            _ => app.ui.backstage_page = id.into(),
        }
    }
}

/// A sidebar row: a soft grey pill when selected or hovered.
fn row(ui: &mut Ui, t: &Tokens, label: &str, selected: bool, ink: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if selected {
        ui.painter().rect_filled(r, 6.0, t.pressed);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, tl!(label), font(), ink);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, tl!(label)));
    resp
}

/// Templates or recent documents as cards, with a title bar and Cancel / Create.
fn chooser(app: &mut WordApp, ui: &mut Ui, t: &Tokens, recent: bool) {
    let pick_id = egui::Id::new(("margin_home_pick", recent));
    let query_id = egui::Id::new("margin_home_query");
    let mut pick = ui.data(|d| d.get_temp::<Pick>(pick_id)).unwrap_or_default().0;
    let mut query = ui.data(|d| d.get_temp::<String>(query_id)).unwrap_or_default();
    let mut open: Option<String> = None;

    egui::Panel::top("margin_home_top")
        .exact_size(52.0)
        .frame(egui::Frame::NONE.fill(t.title_bar).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(28, 0)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(tl!(if recent { "Recent documents" } else { "Choose a template" })).font(font()).color(t.text));
                if !recent {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let r = ui.add(egui::TextEdit::singleline(&mut query).hint_text(tl!("Search templates")).desired_width(240.0).font(font()));
                        if r.changed() {
                            ui.data_mut(|d| d.insert_temp(query_id, query.clone()));
                        }
                    });
                }
            });
        });
    egui::Panel::bottom("margin_home_footer")
        .exact_size(56.0)
        .frame(egui::Frame::NONE.fill(t.title_bar).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(24, 0)))
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let label = if recent { "Open" } else { "Create" };
                let ok = egui::Button::new(egui::RichText::new(tl!(label)).font(font()).color(t.on_accent))
                    .fill(t.accent)
                    .min_size(vec2(84.0, 28.0))
                    .corner_radius(7.0);
                if ui.add_enabled(pick.is_some(), ok).clicked() {
                    open = pick.clone();
                }
                let cancel = egui::Button::new(egui::RichText::new(tl!("Cancel")).font(font())).min_size(vec2(84.0, 28.0)).corner_radius(7.0);
                if ui.add(cancel).clicked() {
                    close(app);
                }
            });
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 28, right: 28, top: 24, bottom: 24 }))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let items: Vec<(String, String, Option<String>)> = if recent {
                    app.ui.recent.iter().take(24).map(|p| (p.clone(), file_name(p), Some(folder_and_date(p)))).collect()
                } else {
                    crate::backstage::matching_templates(&query)
                        .into_iter()
                        .map(|(label, id)| (id.to_string(), tl!(label).to_string(), None))
                        .collect()
                };
                if items.is_empty() {
                    let msg = if recent { "Documents you open will show up here." } else { "No templates match your search." };
                    ui.label(egui::RichText::new(tl!(msg)).font(font()).color(t.text_dim));
                    return;
                }
                if !recent && pick.is_none() {
                    pick = items.first().map(|(k, _, _)| k.clone());
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(24.0, 28.0);
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

/// One page-shaped card with its label (and folder/date for recent documents) underneath.
#[allow(clippy::too_many_arguments)]
fn card(app: &mut WordApp, ui: &mut Ui, t: &Tokens, key: &str, label: &str, sub: Option<&str>, selected: bool, recent: bool) -> egui::Response {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.set_width(CARD_W);
        let (r, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::click());
        // Soft shadow, then the paper.
        let lift = if resp.hovered() { 3.0 } else { 1.0 };
        ui.painter().rect_filled(r.translate(vec2(0.0, lift)).expand(1.0), 8.0, Color32::from_black_alpha(if resp.hovered() { 40 } else { 22 }));
        ui.painter().rect_filled(r, 8.0, Color32::WHITE);
        let tex = if recent { recent_thumb(app, ui, key) } else { template_thumb(app, ui, key) };
        match tex {
            Some(h) => {
                ui.painter().add(
                    egui::epaint::RectShape::filled(r, 8.0, Color32::WHITE).with_texture(h.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
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
        let ring = if selected { Stroke::new(3.0, t.accent) } else { Stroke::new(1.0, Color32::from_black_alpha(26)) };
        ui.painter().rect_stroke(r, 8.0, ring, egui::StrokeKind::Outside);
        ui.add_space(6.0);
        let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), font(), t.text);
        job.wrap = egui::text::TextWrapping::truncate_at_width(CARD_W);
        job.halign = egui::Align::Center;
        ui.vertical_centered(|ui| {
            ui.label(job);
            if let Some(s) = sub {
                ui.label(egui::RichText::new(s).font(font()).color(t.group_label));
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
    let img = wordcraft_render::render_page(&s.doc, page, CARD_W / page.w * ppp, &crate::canvas::screen_render_options());
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
