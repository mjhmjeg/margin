//! O7 (OPTIMO.md §8, S1–S4): Structure's "Shape" in the left desk margin.
//!
//! * **S1** 264 wide: "6 sections · 370 words", then one block per heading: the heading, the
//!   co-writer's one-line purpose (`text_dim`) and a 3 pt length bar (`border_strong`, width by
//!   words). The caret's section has the `checked` fill. A click goes to the heading; dragging a
//!   block onto another is the person's own tracked move ([`wordcraft_engine::structure`]).
//! * **S2** A suggested move sits where the section would go, under a dotted rule: "Move
//!   “Membership” here?" and the reader's reason; Move it · Not now. Move it lands one tracked
//!   move by the AI author (`cowriter::apply`), nothing before.
//! * **S3** An empty or thin section links to I'm stuck (`stuck_view::enter`).
//! * **S4** A missing section is a dashed block: "Costs — a reader will look for…"; Add heading
//!   (heading text only, one tracked insert by the AI author) · Not needed.
//!
//! The co-writer's observations are ordinary notes in the right margin. Accept all lands every
//! open suggestion in turn.

use egui::{Color32, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::assistant_edits::{ScopeKind, ScopeText};
use wordcraft_engine::cowriter::{self, Output, Ran, Verb};
use wordcraft_engine::doc::{Path, Pos};
use wordcraft_engine::structure::{self as st, Section};

use crate::WordApp;
use crate::mac_controls as mc;
use crate::theme::Tokens;

/// The Shape column's width, and its distance from the page.
pub const SHAPE_W: f32 = 264.0;
const FROM_PAGE: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Open,
    Done,
    Dismissed,
}

/// A heading move or a missing heading, waiting for the person.
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub output: Output,
    pub why: String,
    pub state: State,
}

/// The view of the document's last Structure run.
pub struct StructureView {
    pub doc: u64,
    pub name: String,
    scope: ScopeText,
    pub purposes: Vec<(String, String)>,
    pub suggestions: Vec<Suggestion>,
}

enum Act {
    Go(usize),
    Stuck(usize),
    Accept(usize),
    Dismiss(usize),
    AcceptAll,
    Drag(usize, usize),
    Close,
}

/// Called by `cowriter::start` once the run is certain: open the view (the outline shows at once).
pub fn begin(app: &mut WordApp, scope: &ScopeText) {
    let name = app.ui.assistant.name();
    app.cowriter.structure =
        Some(StructureView { doc: app.session.document_id(), name, scope: scope.clone(), purposes: Vec::new(), suggestions: Vec::new() });
    // One thing on the left: Format for me's Before thumbnail goes.
    app.cowriter.format = None;
    crate::mac_ui::set_margin(app, true);
}

/// The run's answer (`cowriter::land` hands it over): moves and missing headings are kept here as
/// suggestions, not landed; observations go on as notes. `raw` is the answer as sent, for the
/// purposes and reasons the runner's outputs don't carry.
pub fn take(app: &mut WordApp, mut ran: Ran, raw: Option<&str>) -> Ran {
    let doc = app.session.document_id();
    let answer = raw.and_then(|t| st::parse(t).ok()).unwrap_or_default();
    let Some(v) = app.cowriter.structure.as_mut().filter(|v| v.doc == doc) else {
        ran.outputs.retain(|o| matches!(o, Output::Comment { .. }));
        return ran;
    };
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    v.purposes = answer.purposes.clone();
    v.suggestions.clear();
    for o in std::mem::take(&mut ran.outputs) {
        let why = match &o {
            Output::HeadingMove { heading, .. } => answer.moves.iter().find(|m| same(&m.heading, heading)).map(|m| m.why.clone()),
            Output::HeadingInsert { text, .. } => answer.missing.iter().find(|m| same(&m.heading, text)).map(|m| m.why.clone()),
            _ => {
                ran.outputs.push(o);
                continue;
            }
        };
        v.suggestions.push(Suggestion { output: o, why: why.unwrap_or_default(), state: State::Open });
    }
    ran
}

pub fn active(app: &WordApp) -> bool {
    let doc = app.session.document_id();
    app.cowriter.structure.as_ref().is_some_and(|v| v.doc == doc)
}

/// The heading a suggestion is about.
fn about(o: &Output) -> &str {
    match o {
        Output::HeadingMove { heading, .. } => heading,
        Output::HeadingInsert { text, .. } => text,
        _ => "",
    }
}

/// The section the caret is in.
fn current(app: &WordApp, list: &[Section]) -> Option<usize> {
    let here = app.session.sel.focus.path.0.first().copied().unwrap_or(0) as usize;
    list.iter().rposition(|x| x.block <= here)
}

/// Where a suggestion shows: before section index `k` of the outline (`list.len()` = at the end).
fn slot(list: &[Section], o: &Output) -> usize {
    match o {
        Output::HeadingMove { before, .. } => st::find(list, before).and_then(|b| list.iter().position(|x| x.block == b.block)),
        Output::HeadingInsert { after, .. } => st::find(list, after).map(|a| list.iter().position(|x| x.block >= a.range_end).unwrap_or(list.len())),
        _ => None,
    }
    .unwrap_or(list.len())
}

/// Points of a rounded rectangle's outline, closed, for a dashed stroke.
fn rounded(r: Rect, radius: f32) -> Vec<egui::Pos2> {
    let rad = radius.min(r.width() / 2.0).min(r.height() / 2.0);
    let corners = [
        (pos2(r.max.x - rad, r.min.y + rad), -90.0f32),
        (pos2(r.max.x - rad, r.max.y - rad), 0.0),
        (pos2(r.min.x + rad, r.max.y - rad), 90.0),
        (pos2(r.min.x + rad, r.min.y + rad), 180.0),
    ];
    let mut pts = Vec::with_capacity(40);
    for (c, start) in corners {
        for i in 0..=6 {
            let a = (start + 15.0 * i as f32).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    if let Some(first) = pts.first().copied() {
        pts.push(first);
    }
    pts
}

/// One line of text at `y` (wrapped to `w`, at most `rows`); returns its height.
fn text(painter: &egui::Painter, at: egui::Pos2, w: f32, s: &str, color: Color32, rows: usize) -> f32 {
    let mut job = egui::text::LayoutJob::single_section(s.to_string(), egui::TextFormat { font_id: mc::font(), color, ..Default::default() });
    job.wrap.max_width = w.max(20.0);
    job.wrap.max_rows = rows;
    let g = painter.layout_job(job);
    let h = g.size().y;
    painter.galley(at, g, color);
    h
}

/// Links in a row at `at`; returns (height, the index clicked).
fn links(ui: &mut Ui, painter: &egui::Painter, at: egui::Pos2, labels: &[&str], salt: usize) -> (f32, Option<usize>) {
    let t = Tokens::get(ui.ctx());
    let mut x = at.x;
    let mut h = 0.0f32;
    let mut hit = None;
    for (i, l) in labels.iter().enumerate() {
        let g = painter.layout_no_wrap(tl!(l).to_string(), mc::font(), t.accent_text);
        let r = Rect::from_min_size(pos2(x, at.y), g.size());
        h = h.max(g.size().y);
        x = r.max.x + 14.0;
        let resp =
            ui.interact(r.expand(2.0), ui.id().with(("structure_link", salt, i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.hovered() {
            painter.hline(r.x_range(), r.max.y, Stroke::new(1.0, t.accent_text));
        }
        painter.galley(r.min, g, t.accent_text);
        let name = tl!(l);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
        if resp.clicked() {
            hit = Some(i);
        }
    }
    (h, hit)
}

/// The Shape column left of the page (`rects`: the pages on screen).
pub fn paint(app: &mut WordApp, ui: &mut Ui, painter: &egui::Painter, rects: &[Rect]) {
    if !active(app) {
        return;
    }
    let Some(page) = rects.first() else { return };
    let clip = ui.clip_rect();
    let left = (page.min.x - FROM_PAGE - SHAPE_W).max(clip.min.x + 12.0);
    let w = (page.min.x - FROM_PAGE - left).min(SHAPE_W);
    // ponytail: no Shape on a desk narrower than a readable column; the notes still show.
    if w < 160.0 {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let working = app.cowriter.working();
    let list = st::outline(&app.session);
    let here = current(app, &list);
    let Some(v) = app.cowriter.structure.as_ref() else { return };
    let mut acts: Vec<Act> = Vec::new();
    let mut y = clip.min.y + 12.0;
    let x = left;
    // Header: "6 sections · 370 words" · Close.
    let head =
        crate::i18n::fmt(tl!("{n} sections · {w} words"), &[("n", &list.len().to_string()), ("w", &st::total_words(&app.session).to_string())]);
    let hh = text(painter, pos2(x, y), w - 60.0, &head, t.text, 1);
    let (_, hit) = links(ui, painter, pos2(x + w - 40.0, y), &["Close"], usize::MAX);
    if hit.is_some() {
        acts.push(Act::Close);
    }
    y += hh + 4.0;
    if working {
        y += text(painter, pos2(x, y), w, tl!("Reading the shape…"), m.assistant, 1) + 6.0;
    }
    if list.is_empty() {
        y += text(painter, pos2(x, y), w, tl!("No headings yet. Headings give the document its shape."), t.text_dim, 3) + 6.0;
    }
    let max_words = list.iter().map(|x| x.words).max().unwrap_or(0).max(1);
    let open: Vec<(usize, usize)> =
        v.suggestions.iter().enumerate().filter(|(_, s)| s.state == State::Open).map(|(i, s)| (i, slot(&list, &s.output))).collect();
    let mut blocks: Vec<Rect> = Vec::new();
    let mut drop: Option<(usize, egui::Pos2)> = None;
    let bottom = clip.max.y - 8.0;
    for k in 0..=list.len() {
        // S2 / S4 before section k.
        for (i, _) in open.iter().filter(|(_, at)| *at == k) {
            if y > bottom {
                break;
            }
            let Some(sg) = v.suggestions.get(*i) else { continue };
            match &sg.output {
                Output::HeadingMove { heading, .. } => {
                    crate::review_panel::mark_line(painter, pos2(x, y + 2.0), pos2(x + w, y + 2.0), m.assistant, true);
                    y += 8.0;
                    let q = crate::i18n::fmt(tl!("Move “{h}” here?"), &[("h", &crate::review_panel::clip(heading, 40))]);
                    let line = if sg.why.is_empty() { q } else { format!("{q} {}", sg.why) };
                    y += text(painter, pos2(x, y), w, &line, t.text, 4) + 2.0;
                    let (h, hit) = links(ui, painter, pos2(x, y), &["Move it", "Not now"], *i);
                    match hit {
                        Some(0) => acts.push(Act::Accept(*i)),
                        Some(_) => acts.push(Act::Dismiss(*i)),
                        None => {}
                    }
                    y += h + 10.0;
                }
                Output::HeadingInsert { text: heading, .. } => {
                    let line = if sg.why.is_empty() { heading.clone() } else { format!("{heading} — {}", sg.why) };
                    let galley_h = {
                        let mut job = egui::text::LayoutJob::single_section(
                            line.clone(),
                            egui::TextFormat { font_id: mc::font(), color: t.text, ..Default::default() },
                        );
                        job.wrap.max_width = w - 20.0;
                        job.wrap.max_rows = 4;
                        painter.layout_job(job).size().y
                    };
                    let r = Rect::from_min_size(pos2(x, y), vec2(w, galley_h + 36.0));
                    painter.add(Shape::dashed_line(&rounded(r, 8.0), Stroke::new(1.5, t.border_strong), 4.0, 3.0));
                    text(painter, pos2(x + 10.0, y + 8.0), w - 20.0, &line, t.text, 4);
                    let (_, hit) = links(ui, painter, pos2(x + 10.0, y + 12.0 + galley_h), &["Add heading", "Not needed"], *i);
                    match hit {
                        Some(0) => acts.push(Act::Accept(*i)),
                        Some(_) => acts.push(Act::Dismiss(*i)),
                        None => {}
                    }
                    y = r.max.y + 8.0;
                }
                _ => {}
            }
        }
        let Some(sec) = list.get(k) else { break };
        if y > bottom {
            break;
        }
        // S1: one block.
        let indent = 12.0 * f32::from(sec.level.min(3));
        let bx = x + indent;
        let bw = w - indent;
        let purpose = v.purposes.iter().find(|(h, _)| h.trim().eq_ignore_ascii_case(&sec.heading)).map(|(_, p)| p.as_str());
        let h_head = painter.layout(sec.heading.clone(), mc::font(), t.text, bw - 16.0).size().y.min(36.0);
        let h_purpose = purpose.map_or(0.0, |p| painter.layout(p.to_string(), mc::font(), t.text_dim, bw - 16.0).size().y.min(36.0) + 2.0);
        let r = Rect::from_min_size(pos2(bx, y), vec2(bw, 8.0 + h_head + h_purpose + 6.0 + 3.0 + 8.0));
        let resp = ui.interact(r, ui.id().with(("structure_block", k)), Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::Grab);
        if here == Some(k) {
            painter.rect_filled(r, 6.0, t.checked);
        } else if resp.hovered() {
            painter.rect_filled(r, 6.0, t.hover);
        }
        let mut ty = y + 8.0;
        ty += text(painter, pos2(bx + 8.0, ty), bw - 16.0, &sec.heading, t.text, 2) + 2.0;
        if let Some(p) = purpose {
            ty += text(painter, pos2(bx + 8.0, ty), bw - 16.0, p, t.text_dim, 2) + 2.0;
        }
        let bar = ((bw - 16.0) * sec.words as f32 / max_words as f32).max(if sec.words > 0 { 3.0 } else { 0.0 });
        painter.rect_filled(Rect::from_min_size(pos2(bx + 8.0, ty + 4.0), vec2(bar, 3.0)), 1.5, t.border_strong);
        let name = sec.heading.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.as_str()));
        if resp.clicked() {
            acts.push(Act::Go(k));
        }
        if resp.dragged()
            && let Some(p) = ui.ctx().pointer_interact_pos()
        {
            painter.text(p + vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, &sec.heading, mc::font(), t.text_dim);
        }
        if resp.drag_stopped()
            && let Some(p) = ui.ctx().pointer_interact_pos()
        {
            drop = Some((k, p));
        }
        blocks.push(r);
        y = r.max.y + 4.0;
        // S3: empty or thin.
        if sec.thin() {
            let label = if sec.empty() {
                tl!("Empty — get unstuck").to_string()
            } else {
                crate::i18n::fmt(tl!("Only {n} words — get unstuck"), &[("n", &sec.words.to_string())])
            };
            let (h, hit) = links(ui, painter, pos2(bx + 8.0, y), &[label.as_str()], 10_000 + k);
            if hit.is_some() {
                acts.push(Act::Stuck(k));
            }
            y += h + 6.0;
        }
    }
    // A block dropped onto another goes before it (known once all are laid out).
    if let Some((from, p)) = drop
        && let Some(to) = blocks.iter().position(|b| b.y_range().contains(p.y))
        && to != from
    {
        acts.push(Act::Drag(from, to));
    }
    if open.len() >= 2 && y < bottom {
        let (_, hit) = links(ui, painter, pos2(x, y + 4.0), &["Accept all"], 20_000);
        if hit.is_some() {
            acts.push(Act::AcceptAll);
        }
    }
    for a in acts {
        act(app, a, &list);
    }
}

/// Land suggestion `i` (one tracked move or heading by the AI author).
fn accept(app: &mut WordApp, i: usize) -> Result<(), String> {
    let Some(v) = app.cowriter.structure.as_ref() else { return Err("Structure isn't open".into()) };
    let Some(sg) = v.suggestions.get(i).filter(|s| s.state == State::Open) else { return Err("that suggestion is gone".into()) };
    let (o, name, scope) = (sg.output.clone(), v.name.clone(), v.scope.clone());
    crate::assistant_link::sync_policy(app);
    let landed = cowriter::apply(&mut app.session, &name, Verb::Structure, &scope, vec![o]);
    if landed.applied.is_empty() {
        return Err(landed.lines.first().cloned().unwrap_or_else(|| tl!("Nothing changed.").into()));
    }
    app.ui.assistant.remember_name(&name);
    if let Some(s) = app.cowriter.structure.as_mut().and_then(|v| v.suggestions.get_mut(i)) {
        s.state = State::Done;
    }
    app.canvas.nav_to_caret = true;
    Ok(())
}

fn act(app: &mut WordApp, a: Act, list: &[Section]) {
    match a {
        Act::Go(k) => {
            if let Some(sec) = list.get(k) {
                app.session.sel = wordcraft_engine::Selection::caret(Pos::body(sec.block, 0));
                app.canvas.nav_to_caret = true;
                app.canvas.want_focus = true;
            }
        }
        Act::Stuck(k) => {
            if let Some(sec) = list.get(k) {
                // The last paragraph of the section's own text (the heading when it has none).
                let last = sec.end.saturating_sub(1).max(sec.block);
                if let Err(e) = crate::stuck_view::enter(app, Some(Path::top(last))) {
                    app.status(e);
                }
            }
        }
        Act::Accept(i) => {
            if let Err(e) = accept(app, i) {
                app.status(e);
            }
        }
        Act::Dismiss(i) => {
            if let Some(s) = app.cowriter.structure.as_mut().and_then(|v| v.suggestions.get_mut(i)) {
                s.state = State::Dismissed;
            }
        }
        Act::AcceptAll => {
            let n = app.cowriter.structure.as_ref().map_or(0, |v| v.suggestions.len());
            let mut why = Vec::new();
            for i in 0..n {
                let open = app.cowriter.structure.as_ref().and_then(|v| v.suggestions.get(i)).is_some_and(|s| s.state == State::Open);
                if open && let Err(e) = accept(app, i) {
                    why.push(e);
                }
            }
            if let Some(e) = why.first() {
                app.status(e.clone());
            }
        }
        Act::Drag(from, to) => {
            let (Some(a), Some(b)) = (list.get(from), list.get(to)) else { return };
            // The person's own tracked move.
            match st::move_section(&mut app.session, None, &a.heading, &b.heading) {
                Ok(_) => app.canvas.nav_to_caret = true,
                Err(e) => app.status(e),
            }
        }
        Act::Close => app.cowriter.structure = None,
    }
}

/// `ui.assistant.structureView` (tests and screenshots, no model): `{"answer": "<json text>"}`
/// opens the view and reads that text as the model's answer through the real reader and guards
/// (observations land as notes); `{"accept": i}`, `{"dismiss": i}`, `{"acceptAll": true}`,
/// `{"stuck": k}` (section k), `{"close": true}` click as the person would. Returns the state.
pub fn control(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    if let Some(answer) = p.get("answer").and_then(Value::as_str) {
        crate::cowriter::run_with_answer(app, Verb::Structure, ScopeKind::Document, answer)?;
    }
    if !active(app) {
        return Err("Structure is not open in this document".into());
    }
    let list = st::outline(&app.session);
    let index = |k: &str| p.get(k).and_then(Value::as_u64).map(|i| usize::try_from(i).unwrap_or(usize::MAX));
    if let Some(i) = index("accept") {
        accept(app, i)?;
    }
    if let Some(i) = index("dismiss") {
        act(app, Act::Dismiss(i), &list);
    }
    if p.get("acceptAll").and_then(Value::as_bool) == Some(true) {
        act(app, Act::AcceptAll, &list);
    }
    if let Some(k) = index("stuck") {
        act(app, Act::Stuck(k), &list);
    }
    if p.get("close").and_then(Value::as_bool) == Some(true) {
        app.cowriter.structure = None;
    }
    let list = st::outline(&app.session);
    Ok(match app.cowriter.structure.as_ref() {
        Some(v) => json!({
            "open": true,
            "sections": list.iter().map(|x| json!({"heading": x.heading, "level": x.level + 1, "words": x.words})).collect::<Vec<_>>(),
            "suggestions": v.suggestions.iter().map(|s| json!({
                "about": about(&s.output),
                "kind": if matches!(s.output, Output::HeadingMove { .. }) { "move" } else { "missing" },
                "why": s.why,
                "state": match s.state { State::Open => "open", State::Done => "done", State::Dismissed => "dismissed" },
            })).collect::<Vec<_>>(),
        }),
        None => json!({"open": false}),
    })
}
