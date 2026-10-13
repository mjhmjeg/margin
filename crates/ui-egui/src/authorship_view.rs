//! View › Show Authorship (HANDOFF §10, V1–V4; Margin layout only). While the view is on:
//!
//! * V1 a pill at the top of the desk with the legend (People solid · assistant dotted), a note
//!   that pending suggestions are counted apart, and Hide;
//! * V2 accepted assistant text keeps `assistant_tint` and a dotted underline on the page
//!   (`wordcraft_layout::display::AuthorshipDisplay`, fed from here);
//! * V3 a chip in the left desk margin beside every paragraph with at least
//!   [`CHIP_MIN_SHARE`] of its characters written by an assistant: "Claude 41 %";
//! * V4 the status bar shows the document's split: "People 88 % · Claude 12 %", plus
//!   "· 3 % pending" while suggestions are waiting.
//!
//! Numbers count characters (no placeholders, no tracked deletions) over the body, headers,
//! footers, notes and text boxes. **Pending suggestions are not AI text yet:** they are counted
//! separately (status bar) and never in a chip. Everything is computed by the engine
//! (`Session::authorship`) once per document revision and read here.

use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_doc::StoryRef;
use wordcraft_doc::authorship::{Attribution, ParaShare};
use wordcraft_engine::authorship::display_name;
use wordcraft_layout::display::AuthorshipDisplay;
use wordcraft_layout::{DocLayout, Placed};

use crate::WordApp;
use crate::review_panel::mark_line;
use crate::skin::{ASSISTANT_PAGE, MarginTokens};
use crate::theme::{Tokens, regular};

/// A paragraph gets a chip from this share of assistant characters on.
pub const CHIP_MIN_SHARE: f32 = 0.10;
/// The tint's opacity behind accepted assistant text (as `ASSISTANT_TINT`).
const TINT_ALPHA: u8 = 23;

/// Whether the view is on in a layout that shows it (never in Classic).
pub fn on(app: &WordApp) -> bool {
    app.session.view.authorship && crate::mac_ui::active(app)
}

/// The split for the current document, computed once per revision.
pub fn attribution(app: &mut WordApp) -> Arc<Attribution> {
    let key = (app.session.document_id(), app.session.rev(), app.session.assistant_authors.len());
    if let Some((k, a)) = &app.authorship_cache
        && *k == key
    {
        return a.clone();
    }
    let a = Arc::new(app.session.authorship());
    app.authorship_cache = Some((key, a.clone()));
    a
}

fn rgb(c: Color32) -> wordcraft_doc::Rgb {
    wordcraft_doc::Rgb(c.r(), c.g(), c.b())
}

/// What the page renderer needs while the view is on.
pub fn display(app: &mut WordApp) -> Option<AuthorshipDisplay> {
    if !on(app) {
        return None;
    }
    Some(AuthorshipDisplay { shares: attribution(app), color: rgb(ASSISTANT_PAGE), tint: (rgb(ASSISTANT_PAGE), TINT_ALPHA) })
}

/// Whole-number percentages that add up to 100: people, then each assistant (grouped by the
/// name on its changes, shown without an `AI · ` prefix), then pending suggestions when any.
pub fn percentages(a: &Attribution) -> Vec<(String, u32)> {
    let total = a.total_chars.max(1) as f64;
    let pct = |n: usize| ((n as f64) * 100.0 / total).round() as u32;
    let mut out: Vec<(String, u32)> = Vec::new();
    // Several names can show the same (an "AI · " prefix dropped): add them up.
    for (name, n) in &a.authors {
        let label = display_name(name).to_string();
        match out.iter_mut().find(|(l, _)| *l == label) {
            Some((_, p)) => *p += pct(*n),
            None => out.push((label, pct(*n))),
        }
    }
    if a.pending_chars > 0 {
        out.push((crate::tl!("pending").to_string(), pct(a.pending_chars)));
    }
    let used: u32 = out.iter().map(|(_, p)| *p).sum();
    let people = if a.total_chars == 0 { 100 } else { 100u32.saturating_sub(used).max(if a.people_chars() > 0 { 1 } else { 0 }) };
    out.insert(0, (crate::tl!("People").to_string(), people));
    out
}

/// V4: "People 88 % · Claude 12 %" (" · 3 % pending" while suggestions wait), or `None` while
/// the view is off.
pub fn status_text(app: &mut WordApp) -> Option<String> {
    if !on(app) {
        return None;
    }
    let a = attribution(app);
    let parts: Vec<String> = percentages(&a)
        .into_iter()
        .map(|(label, p)| if label == crate::tl!("pending") { format!("{p} % {label}") } else { format!("{label} {p} %") })
        .collect();
    Some(parts.join(" · "))
}

/// V3: the chip for a paragraph, when at least [`CHIP_MIN_SHARE`] of it is an assistant's.
pub fn chip_label(share: &ParaShare) -> Option<String> {
    if share.total_chars == 0 || share.ai_chars == 0 {
        return None;
    }
    let f = share.ai_chars as f32 / share.total_chars as f32;
    if f < CHIP_MIN_SHARE {
        return None;
    }
    let author = display_name(share.main_author().unwrap_or(""));
    let author = if author.is_empty() { crate::tl!("Assistant").to_string() } else { author.to_string() };
    Some(format!("{author} {} %", (f * 100.0).round() as u32))
}

/// Names of the assistants with text in the document (shown without an `AI · ` prefix), or the
/// configured one while there is none.
fn legend_names(a: &Attribution) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for n in a.authors.keys() {
        let d = display_name(n).to_string();
        if !names.contains(&d) {
            names.push(d);
        }
    }
    if names.is_empty() {
        names.push(display_name(&crate::review_panel::assistant_author_name()).to_string());
    }
    names
}

/// V1 and V3 on the desk: the pill at the top of the viewport and the chips beside paragraphs.
pub fn paint(app: &mut WordApp, ui: &mut Ui, painter: &egui::Painter, rects: &[Rect], layout: &DocLayout, scale: f32) {
    if !on(app) {
        return;
    }
    let a = attribution(app);
    let t = Tokens::get(ui.ctx());
    let m = MarginTokens::get(ui.ctx());
    let clip = ui.clip_rect();
    chips(&a, ui, painter, rects, layout, scale, &m, clip);
    if pill(&a, ui, painter, &t, &m, clip) {
        let _ = app.run("view.authorship", json!({"value": false}));
    }
}

#[allow(clippy::too_many_arguments)]
fn chips(a: &Attribution, ui: &mut Ui, painter: &egui::Painter, rects: &[Rect], layout: &DocLayout, scale: f32, m: &MarginTokens, clip: Rect) {
    let font = regular(11.0);
    for (pi, pr) in rects.iter().enumerate() {
        let Some(page) = layout.pages.get(pi) else { continue };
        if !pr.intersects(clip) {
            continue;
        }
        let ps = crate::canvas::page_screen_scale(*pr, page, scale);
        let mut last_bottom = f32::MIN;
        for it in &page.items {
            let Placed::Lines { story: StoryRef::Body, path, l0: 0, y, turn, .. } = it else { continue };
            if turn.is_turned() {
                continue;
            }
            let Some(share) = a.para(StoryRef::Body, path) else { continue };
            let Some(label) = chip_label(share) else { continue };
            let g = painter.layout_no_wrap(label.clone(), font.clone(), m.assistant);
            let w = 8.0 + 10.0 + 5.0 + g.size().x + 8.0;
            let top = pr.min.y + y * ps;
            if top < last_bottom + 2.0 {
                continue; // a chip per line of the desk: don't stack them
            }
            let right = pr.min.x + page.body.x * ps - 12.0;
            let x0 = (right - w).max(clip.min.x + 4.0);
            let r = Rect::from_min_size(pos2(x0, top), vec2(w, 20.0));
            if !r.intersects(clip) {
                continue;
            }
            last_bottom = r.max.y;
            painter.rect(r, 10.0, m.popover_bg, Stroke::new(1.0, m.popover_border), egui::StrokeKind::Inside);
            let my = r.center().y;
            mark_line(painter, pos2(r.min.x + 8.0, my), pos2(r.min.x + 18.0, my), m.assistant, true);
            painter.galley(pos2(r.min.x + 23.0, my - g.size().y / 2.0), g, m.assistant);
            let resp = ui.interact(r, ui.id().with(("authorship_chip", pi, path.0.clone())), Sense::hover());
            let who = display_name(share.main_author().unwrap_or("")).to_string();
            let tip = crate::i18n::fmt(
                crate::tl!("{ai} of {total} characters written by {who}; the rest by people. Pending suggestions are not counted."),
                &[("ai", &share.ai_chars.to_string()), ("total", &share.total_chars.to_string()), ("who", &who)],
            );
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.clone()));
            resp.on_hover_text(tip);
        }
    }
}

/// The pill; `true` when Hide was clicked.
fn pill(a: &Attribution, ui: &mut Ui, painter: &egui::Painter, t: &Tokens, m: &MarginTokens, clip: Rect) -> bool {
    let font = crate::mac_controls::font();
    let names = legend_names(a);
    // Parts: (text, colour, swatch dotted?) — a swatch is drawn before the text when given.
    let mut parts: Vec<(String, Color32, Option<bool>)> = vec![(crate::tl!("People").to_string(), t.text, Some(false))];
    for n in names {
        parts.push((n, t.text, Some(true)));
    }
    parts.push((crate::tl!("Pending suggestions not counted").to_string(), t.text_dim, None));
    let hide = crate::tl!("Hide").to_string();
    let galleys: Vec<_> = parts.iter().map(|(s, c, _)| painter.layout_no_wrap(s.clone(), font.clone(), *c)).collect();
    let hide_g = painter.layout_no_wrap(hide.clone(), font.clone(), t.accent);
    let sep = 20.0;
    let swatch = 14.0 + 6.0;
    let mut w = 16.0;
    for (g, (_, _, sw)) in galleys.iter().zip(parts.iter()) {
        w += if sw.is_some() { swatch } else { 0.0 } + g.size().x + sep;
    }
    w += hide_g.size().x + 16.0;
    let h = 30.0;
    let r = Rect::from_min_size(pos2(clip.center().x - w / 2.0, clip.min.y + 10.0), vec2(w, h));
    painter.add(m.popover_shadow.as_shape(r, h / 2.0));
    painter.rect(r, h / 2.0, m.popover_bg, Stroke::new(1.0, m.popover_border), egui::StrokeKind::Inside);
    let mut x = r.min.x + 16.0;
    let cy = r.center().y;
    for (g, (_, _, sw)) in galleys.into_iter().zip(parts.iter()) {
        if let Some(dotted) = sw {
            let (c, d) = if *dotted { (m.assistant, true) } else { (m.author_1, false) };
            mark_line(painter, pos2(x, cy), pos2(x + 14.0, cy), c, d);
            x += swatch;
        }
        let gw = g.size().x;
        painter.galley(pos2(x, cy - g.size().y / 2.0), g, t.text);
        x += gw;
        painter.text(pos2(x + sep / 2.0, cy), egui::Align2::CENTER_CENTER, "·", font.clone(), t.text_dim);
        x += sep;
    }
    let hr = Rect::from_min_size(pos2(x, cy - 10.0), vec2(hide_g.size().x, 20.0));
    let resp = ui.interact(hr.expand2(vec2(6.0, 0.0)), ui.id().with("authorship_hide"), Sense::click());
    let color = if resp.hovered() { t.accent_text } else { t.accent };
    painter.galley(pos2(x, cy - hide_g.size().y / 2.0), hide_g, color);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hide.clone()));
    resp.on_hover_text(crate::tl!("View › Show Authorship switches it back on")).clicked()
}

/// A dotted or solid 2 pt swatch, for legends elsewhere.
pub fn swatch(painter: &egui::Painter, at: Pos2, dotted: bool, color: Color32) {
    mark_line(painter, at, pos2(at.x + 14.0, at.y), color, dotted);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use wordcraft_engine::Session;

    fn app_with_ai_text() -> WordApp {
        let mut a = WordApp::new(
            Session::new(wordcraft_doc::Document::from_text("The cat sat on the mat.\nA second line by a person.")),
            Services::default(),
        );
        a.session.author = "Martijn".into();
        a.session.run("select.range", &json!({"anchor": {"block": 0, "off": 11}, "focus": {"block": 0, "off": 11}})).unwrap();
        a.session.run_as_assistant("Claude", "text.insert", &json!({"text": " happily on the mat", "raw": true})).unwrap();
        a
    }

    #[test]
    fn percentages_add_up_and_pending_is_counted_apart() {
        let mut a = app_with_ai_text();
        a.run("view.authorship", json!({})).unwrap();
        // 68 characters, 19 of them pending: nothing accepted yet.
        let at = attribution(&mut a);
        assert_eq!((at.total_chars, at.pending_chars, at.ai_chars), (68, 19, 0));
        assert_eq!(percentages(&at), vec![("People".to_string(), 72), ("pending".to_string(), 28)]);
        assert_eq!(status_text(&mut a).as_deref(), Some("People 72 % · 28 % pending"));
        a.run("review.acceptAll", json!({"author": "Claude"})).unwrap();
        let at = attribution(&mut a);
        assert_eq!((at.total_chars, at.pending_chars, at.ai_chars), (68, 0, 19));
        assert_eq!(percentages(&at), vec![("People".to_string(), 72), ("Claude".to_string(), 28)]);
        assert_eq!(status_text(&mut a).as_deref(), Some("People 72 % · Claude 28 %"));
        // V3: the first paragraph is 19 of 42 the assistant's; the second has no chip.
        let p0 = at.para(StoryRef::Body, &wordcraft_doc::Path::top(0)).unwrap();
        assert_eq!(chip_label(p0).as_deref(), Some("Claude 45 %"));
        assert_eq!(chip_label(at.para(StoryRef::Body, &wordcraft_doc::Path::top(1)).unwrap()), None);
        // The cache follows the document revision.
        a.run("text.insert", json!({"text": "x"})).unwrap();
        assert_eq!(attribution(&mut a).total_chars, 69);
        // Off: nothing for the status bar, nothing for the page.
        a.run("view.authorship", json!({"value": false})).unwrap();
        assert!(status_text(&mut a).is_none() && display(&mut a).is_none());
    }

    #[test]
    fn a_name_with_the_ai_prefix_is_shown_without_it_and_tiny_shares_get_no_chip() {
        let share = ParaShare { total_chars: 100, ai_chars: 9, authors: [("AI · Mistral Large 4".to_string(), 9)].into(), ..Default::default() };
        assert_eq!(chip_label(&share), None, "under 10 %");
        let share = ParaShare { total_chars: 100, ai_chars: 41, authors: [("AI · Mistral Large 4".to_string(), 41)].into(), ..Default::default() };
        assert_eq!(chip_label(&share).as_deref(), Some("Mistral Large 4 41 %"));
        let at = Attribution { total_chars: 0, ..Default::default() };
        assert_eq!(percentages(&at), vec![("People".to_string(), 100)]);
    }
}
