//! Optimo's Mac layout: one slim toolbar in the title-bar band, a Format inspector on the right
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

/// Optimo's interface settings, saved with the other preferences in `ui.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OptimoPrefs {
    pub classic_ribbon: bool,
    /// Not saved: every window starts on the bare page (W2).
    #[serde(skip)]
    pub inspector: bool,
    /// Open the font list when the inspector next draws (⇧⌘F).
    #[serde(skip)]
    pub font_list: bool,
    pub tab: InspectorTab,
    pub text_width: TextWidth,
    /// Commands run from the command field, most recent first (capped).
    pub command_recent: Vec<String>,
    /// How often each was run there.
    pub command_counts: std::collections::BTreeMap<String, u32>,
}

impl Default for OptimoPrefs {
    fn default() -> Self {
        OptimoPrefs {
            classic_ribbon: false,
            inspector: false,
            font_list: false,
            tab: InspectorTab::Text,
            text_width: TextWidth::Medium,
            command_recent: Vec::new(),
            command_counts: Default::default(),
        }
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
    crate::skin::on() && !app.ui.optimo.classic_ribbon
}

/// Pageless is upstream's Web Layout view.
pub fn pageless(app: &WordApp) -> bool {
    app.session.view.mode == wordcraft_layout::ViewMode::Web
}

const CM: f32 = 72.0 / 2.54;
const ROW_H: f32 = mc::ROW_H;
/// All right-hand panels are 320 wide (Format, Review, later Assistant).
const INSPECTOR_W: f32 = crate::review_panel::PANEL_W;

fn font() -> egui::FontId {
    regular(crate::skin::UI_SIZE)
}

// ---------------------------------------------------------------------------------------------
// Top bar (Optimo W1–W2)

/// Optimo's top bar height (W1).
pub const TOP_BAR_H: f32 = 36.0;
/// The bottom line's height (W4).
pub const BOTTOM_LINE_H: f32 = 32.0;
/// The "Anything ⌘Y" pill (W2).
const PILL_H: f32 = 28.0;

/// W1–W2: the top bar, the desk's colour with no border: the traffic lights, the title centred
/// ("Edited" in `text_dim` while unsaved) and on the right only "Anything ⌘Y" and Co-writer.
/// Everything else is in ⌘Y, the menu bar and the shortcuts.
pub fn toolbar(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("margin_toolbar").exact_size(TOP_BAR_H).frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| {
        let full = ui.max_rect();
        let drag = ui.interact(full, ui.id().with("toolbar_drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }
        // Controls centre on the traffic lights' band (AppKit keeps them in the top 28 pt).
        let cy = full.min.y + MAC_TITLE_BAR / 2.0 + 3.0;

        // Right to left: Co-writer, then the pill.
        let mut x = full.max.x - 12.0;
        let open = crate::assistant_panel::visible(app);
        let toggle = cowriter_toggle(ui, pos2(x, cy), open);
        if toggle.clicked() {
            if crate::review_panel::assistant_enabled() {
                crate::assistant_panel::set_open(app, !open);
            } else {
                // No co-writer set up yet: Settings › Co-writer.
                let _ = app.run("ui.dialog", json!({"name": "options", "pane": "agents"}));
            }
        }
        x = toggle.rect.min.x - 8.0;
        if app.command_field.open {
            // ⌘Y open: the command field takes the pill's place (O2 moves it over the page).
            app.command_field.field_visible = true;
            let rect = Rect::from_min_max(pos2(x - crate::command_field::FIELD_W, cy - PILL_H / 2.0), pos2(x, cy + PILL_H / 2.0));
            crate::command_field::show(app, ui, rect);
            x = rect.min.x;
        } else {
            app.command_field.field_visible = true;
            let pill = anything_pill(ui, pos2(x, cy));
            if pill.clicked() {
                crate::command_field::open(app, "");
            }
            x = pill.rect.min.x;
        }

        // The title, centred on the window (moved left only when the controls need the room).
        let stem = app.title_stem();
        let title = ui.painter().layout_no_wrap(stem, font(), t.text);
        let edited = app.session.dirty.then(|| ui.painter().layout_no_wrap(tl!("Edited").to_string(), font(), t.text_dim));
        let gap = 6.0;
        let w = title.size().x + edited.as_ref().map_or(0.0, |g| gap + g.size().x);
        let left = if app.integrated_titlebar { MAC_CONTENT_LEFT } else { 12.0 };
        let x0 = (full.center().x - w / 2.0).min(x - 16.0 - w).max(full.min.x + left);
        let ty = cy - title.size().y / 2.0;
        let tw = title.size().x;
        ui.painter().with_clip_rect(Rect::from_min_max(pos2(full.min.x + left, full.min.y), pos2(x - 8.0, full.max.y))).galley(
            pos2(x0, ty),
            title,
            t.text,
        );
        if let Some(g) = edited {
            ui.painter().galley(pos2(x0 + tw + gap, ty), g, t.text_dim);
        }
    });
}

/// W2: "Anything ⌘Y", right edge at `right`: 28 high, radius 14, `field_bg`, 1 pt `border_strong`,
/// a search icon, the label in `text_dim` and a `⌘Y` key cap (`checked`, radius 9).
fn anything_pill(ui: &mut Ui, right: egui::Pos2) -> Response {
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let label = ui.painter().layout_no_wrap(tl!("Anything").to_string(), font(), t.text_dim);
    let cap = ui.painter().layout_no_wrap("⌘Y".to_string(), font(), t.text_dim);
    let cap_w = cap.size().x + 12.0;
    let w = 10.0 + 15.0 + 6.0 + label.size().x + 8.0 + cap_w + 5.0;
    let r = Rect::from_min_max(pos2(right.x - w, right.y - PILL_H / 2.0), pos2(right.x, right.y + PILL_H / 2.0));
    let resp = ui.interact(r, ui.id().with("anything_pill"), Sense::click());
    let fill = if resp.hovered() { t.hover } else { m.field_bg };
    ui.painter().rect(r, PILL_H / 2.0, fill, Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 17.5, r.center().y), vec2(15.0, 15.0)), "search", t.text_dim, t.text_dim);
    ui.painter().galley(pos2(r.min.x + 31.0, r.center().y - label.size().y / 2.0), label, t.text_dim);
    let c = Rect::from_min_max(pos2(r.max.x - 5.0 - cap_w, r.min.y + 5.0), pos2(r.max.x - 5.0, r.max.y - 5.0));
    ui.painter().rect_filled(c, 9.0, t.checked);
    ui.painter().galley(c.center() - cap.size() / 2.0, cap, t.text_dim);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Anything ⌘Y"));
    resp.on_hover_text(tl!("Do, find, format, go to — or ask your co-writer"))
}

/// W2: the `assistant` icon in the assistant colour + "Co-writer", right edge at `right`;
/// `checked` while a co-writer view is open.
fn cowriter_toggle(ui: &mut Ui, right: egui::Pos2, on: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let label = tl!("Co-writer");
    let galley = ui.painter().layout_no_wrap(label.to_string(), font(), t.text);
    let w = 10.0 + 17.0 + 6.0 + galley.size().x + 10.0;
    let r = Rect::from_min_max(pos2(right.x - w, right.y - 14.0), pos2(right.x, right.y + 14.0));
    let resp = ui.interact(r, ui.id().with("cowriter_toggle"), Sense::click());
    let fill = if on {
        t.checked
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, mc::RADIUS, fill);
    crate::icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(r.min.x + 18.5, r.center().y), vec2(17.0, 17.0)),
        "assistant",
        m.assistant,
        m.assistant,
    );
    ui.painter().galley(pos2(r.min.x + 33.0, r.center().y - galley.size().y / 2.0), galley, t.text);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, label));
    resp.on_hover_text(tl!("Your co-writer's notes and questions"))
}

/// ⌃⌘R switches between the Mac layout and the Classic ribbon, both ways.
pub fn shortcuts(app: &mut WordApp, ctx: &egui::Context) {
    if !crate::skin::on() {
        return;
    }
    let hit = ctx.input_mut(|i| i.consume_key(egui::Modifiers { ctrl: true, mac_cmd: true, command: true, ..Default::default() }, egui::Key::R));
    if hit {
        app.ui.optimo.classic_ribbon = !app.ui.optimo.classic_ribbon;
    }
    // ⌘Y focuses the command field (the Help menu's accelerator does the same when AppKit sees it
    // first); so does ⌥Q in the Optimo layout (Classic keeps Alt+Q for the old dialog). The letter
    // is matched as typed, so on QWERTZ keyboards this is the key labelled Y.
    let cmd_y = ctx.input_mut(|i| i.consume_key(egui::Modifiers { mac_cmd: true, command: true, ..Default::default() }, egui::Key::Y));
    let alt_q = active(app) && ctx.input_mut(|i| i.consume_key(egui::Modifiers { alt: true, ..Default::default() }, egui::Key::Q));
    if alt_q {
        // Option+Q types "œ" on a US Mac keyboard: drop that text.
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(t) if t == "œ" || t == "Œ")));
    }
    if cmd_y || alt_q {
        crate::command_field::open(app, "");
    }
    // W2: the panels left the window; their keys (also in the View menu): ⇧⌘F Format (Word's
    // "font" key: it opens with the font list), ⇧⌘R Review, ⇧⌘O Outline; ⌥⌘I as in Pages.
    if !active(app) {
        return;
    }
    let shift_cmd = egui::Modifiers { shift: true, mac_cmd: true, command: true, ..Default::default() };
    let cmd_alt = egui::Modifiers { alt: true, mac_cmd: true, command: true, ..Default::default() };
    if ctx.input_mut(|i| i.consume_key(shift_cmd, egui::Key::F)) {
        format_with_font_list(app);
    }
    if ctx.input_mut(|i| i.consume_key(cmd_alt, egui::Key::I)) {
        toggle_panel(app, Panel::Format);
    }
    if ctx.input_mut(|i| i.consume_key(shift_cmd, egui::Key::R)) {
        toggle_panel(app, Panel::Review);
    }
    if ctx.input_mut(|i| i.consume_key(shift_cmd, egui::Key::O)) {
        toggle_panel(app, Panel::Outline);
    }
}

/// The Format inspector's font list (⇧⌘F opens it).
const FONT_POPUP: &str = "margin_font";

/// ⇧⌘F / View › Show Format Inspector: Format on the Text tab with the font list open, where
/// Word's ⇧⌘F would put you; again closes it.
pub fn format_with_font_list(app: &mut WordApp) {
    toggle_panel(app, Panel::Format);
    if app.ui.optimo.inspector {
        app.ui.optimo.tab = InspectorTab::Text;
        app.ui.optimo.font_list = true;
    }
}

/// The panels that open only from ⌘Y, the menu bar or a shortcut (W2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Format,
    Review,
    Outline,
}

/// Open or close a panel; one right panel at a time. Closing returns to the bare page.
pub fn toggle_panel(app: &mut WordApp, panel: Panel) {
    match panel {
        Panel::Format => {
            let open = app.ui.optimo.inspector && !app.session.view.comments_pane && !crate::assistant_panel::visible(app);
            app.ui.optimo.classic_ribbon = false;
            app.ui.optimo.inspector = !open;
            if !open {
                app.session.view.comments_pane = false;
                app.assistant_panel.open = false;
            }
        }
        Panel::Review => {
            let open = !app.session.view.comments_pane;
            app.session.view.comments_pane = open;
            if open {
                app.ui.optimo.inspector = false;
                app.assistant_panel.open = false;
            }
        }
        Panel::Outline => run_from_ribbon(app, "view.navigationPane", json!({})),
    }
}

// ---------------------------------------------------------------------------------------------
// Bottom line (W4)

/// The share of the document's characters people wrote, in whole percent: 100 for an empty
/// document. Pending assistant suggestions and accepted assistant text count as not theirs.
pub fn your_words_percent(a: &wordcraft_doc::authorship::Attribution) -> u32 {
    if a.total_chars == 0 {
        return 100;
    }
    ((a.people_chars() as f64) * 100.0 / (a.total_chars as f64)).round().clamp(0.0, 100.0) as u32
}

/// W4: the 32 pt bottom line, no border, in `text_dim`: "370 words · Your words 97 %" and a
/// 48 × 4 bar on the left, short notices after it, "Focus" on the right.
pub fn status_bar(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("status")
        .exact_size(BOTTOM_LINE_H)
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 16, right: 12, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                let dim = |ui: &mut Ui, s: &str| -> Response {
                    ui.add(egui::Label::new(egui::RichText::new(s).font(font()).color(t.text_dim)).sense(Sense::click()))
                };
                let words = app.cached_word_count();
                let wtxt = if app.session.sel.is_collapsed() || crate::objects::selected(app).is_some() {
                    crate::i18n::fmt(tl!("{words} words"), &[("words", &words.to_string())])
                } else {
                    let sw = wordcraft_doc::count_words(&app.session.selected_text());
                    crate::i18n::fmt(tl!("{selected} of {words} words"), &[("selected", &sw.to_string()), ("words", &words.to_string())])
                };
                if dim(ui, &wtxt).clicked() {
                    let _ = app.run("ui.dialog", json!({"name": "wordCount"}));
                }
                dim(ui, "·");
                let pct = your_words_percent(&crate::authorship_view::attribution(app));
                let label = crate::i18n::fmt(tl!("Your words {pct} %"), &[("pct", &pct.to_string())]);
                let resp = dim(ui, &label).on_hover_text(tl!("Characters you wrote; co-writer suggestions are not yours until you write them"));
                if resp.clicked() {
                    let on = app.session.view.authorship;
                    let _ = app.run("view.authorship", json!({"value": !on}));
                }
                let (bar, _) = ui.allocate_exact_size(vec2(48.0, 4.0), Sense::hover());
                ui.painter().rect_filled(bar, 2.0, t.border_strong);
                ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(48.0 * pct as f32 / 100.0, 4.0)), 2.0, t.text_dim);
                if let Some((msg, at)) = &app.status_msg
                    && crate::now_ms() - at < 6000.0
                {
                    dim(ui, "·");
                    ui.label(egui::RichText::new(msg).font(font()).color(t.accent_text));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let on = app.session.view.focus_mode;
                    let ink = if on { t.text } else { t.text_dim };
                    let resp = ui.add(egui::Label::new(egui::RichText::new(tl!("Focus")).font(font()).color(ink)).sense(Sense::click()));
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, tl!("Focus")));
                    if resp.clicked() {
                        let _ = app.run("view.focus", json!({}));
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
    let m = crate::skin::OptimoTokens::get(ui.ctx());
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
        crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(x + 6.0, r.center().y), vec2(14.0, 14.0)), icon, t.text_dim, t.text_dim);
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
            let cur = app.ui.optimo.tab;
            let sel = tabs.iter().position(|(k, _)| *k == cur).unwrap_or(0);
            if let Some(i) = mc::underline_tabs(ui, &tabs.map(|(_, l)| l), sel, 44.0) {
                app.ui.optimo.tab = tabs.get(i).map_or(cur, |(k, _)| *k);
            }
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.min.y - mc::ROW_GAP, Stroke::new(1.0, t.border));
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.ui.optimo.tab {
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
        if std::mem::take(&mut app.ui.optimo.font_list) {
            mc::request_open(ui.ctx(), FONT_POPUP);
        }
        let fams = app.previews.families();
        if let Some(f) = mc::popup_button(ui, FONT_POPUP, w, &shown, &fams, true) {
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
            let cur = app.ui.optimo.text_width;
            if let Some(i) = segmented(ui, &opts.map(|(_, l)| l), opts.iter().position(|(w, _)| *w == cur)) {
                app.ui.optimo.text_width = opts.get(i).map_or(cur, |(w, _)| *w);
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
    let w = app.ui.optimo.text_width.points().map_or(full, |p| p.min(full));
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
        a.ui.optimo = OptimoPrefs::default();
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
        toggle_panel(h.state_mut(), Panel::Format);
        h.step();
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
        // Medium is 720 pt (no panel open).
        let w = h.state().session.view.web_width;
        assert!((700.0..=720.5).contains(&w), "Medium is {w} pt");
        h.state_mut().ui.optimo.text_width = TextWidth::Narrow;
        h.step();
        h.step();
        assert!((h.state().session.view.web_width - 560.0).abs() < 1.0);
        h.state_mut().ui.optimo.classic_ribbon = true;
        h.step();
        h.step();
        assert!(!active(h.state()));
        assert!(h.query_by_label("Bold").is_some(), "the ribbon is back");
    }

    /// W: a fresh window is the bare page: the two controls, no panel, no ruler, no toolbar
    /// icons or status row; ⌥⌘I / ⌃⌘V / ⌃⌘O open the panels and close them again.
    #[test]
    fn fresh_window_is_the_bare_page_and_keys_open_panels() {
        use egui::accesskit::Role;
        let mut h = mac_harness();
        assert!(h.query_by_label("Anything ⌘Y").is_some() && h.query_by_label("Co-writer").is_some());
        for gone in ["Zoom 100%", "Review", "Format", "Pageless", "Track changes", "Show outline", "Image"] {
            assert!(h.query_by_role_and_label(Role::Button, gone).is_none(), "{gone} left the window");
        }
        let app = h.state();
        assert!(!app.session.view.ruler && !app.ui.optimo.inspector && !app.session.view.comments_pane && !app.session.view.nav_pane);
        assert!(h.query_by_role_and_label(Role::RadioButton, "Center").is_none(), "no inspector");
        let shift_cmd = egui::Modifiers { shift: true, mac_cmd: true, command: true, ..Default::default() };
        let font = h.state_mut().session.run("format.state", &json!({})).ok().and_then(|v| v.get("font").cloned());
        h.key_press_modifiers(shift_cmd, egui::Key::F);
        h.step();
        h.step();
        assert!(h.state().ui.optimo.inspector, "⇧⌘F opens Format");
        assert!(egui::Popup::is_any_open(&h.ctx), "with the font list open");
        let app = h.state_mut();
        assert_eq!(app.session.run("format.state", &json!({})).ok().and_then(|v| v.get("font").cloned()), font, "not Word's font command");
        h.key_press(egui::Key::Escape);
        h.key_press_modifiers(shift_cmd, egui::Key::R);
        h.step();
        assert!(h.state().session.view.comments_pane && !h.state().ui.optimo.inspector, "one right panel at a time");
        h.key_press_modifiers(shift_cmd, egui::Key::R);
        h.key_press_modifiers(shift_cmd, egui::Key::O);
        h.step();
        assert!(!h.state().session.view.comments_pane && h.state().session.view.nav_pane);
        // The pill opens the command field.
        h.get_by_label("Anything ⌘Y").click();
        h.step();
        h.step();
        assert!(h.state().command_field.open);
    }

    #[test]
    fn your_words_share() {
        use wordcraft_doc::authorship::Attribution;
        assert_eq!(your_words_percent(&Attribution::default()), 100);
        let a = Attribution { total_chars: 200, pending_chars: 6, ai_chars: 0, ..Default::default() };
        assert_eq!(your_words_percent(&a), 97);
    }

    #[test]
    fn text_widths() {
        assert_eq!(TextWidth::Narrow.points(), Some(560.0));
        assert_eq!(TextWidth::Full.points(), None);
        assert!(!OptimoPrefs::default().inspector, "every window starts on the bare page");
    }
}
