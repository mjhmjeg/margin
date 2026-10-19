//! O4 (OPTIMO.md §5, T2–T7): what Format for me shows after a run, while the margin view is open.
//!
//! * Right margin (T3–T7): the dotted mark and "Formatted by your co-writer"; a `checked` row
//!   with a lock, "Your words: 0 changed · 412 kept" (the runner's identity check); the Look
//!   switcher (Clean · Classic · Report · Letter, then the person's saved looks) with the model's
//!   "Reads like…" line and "Save as my look…"; What changed, each with Show; the options (Keep
//!   my colours, Add a table of contents, Number the headings); Check lines; Undo all · One by
//!   one · Keep.
//! * Left desk margin (T2): a 264 wide "Before" thumbnail of the page in view, rendered from the
//!   document as it was, and "hold ⌥ to compare": while ⌥ is down the pages show the old
//!   formatting (pictures of the old pages laid over them).
//! * Switching looks, the options and Undo all are undo steps the view counts, so Undo all takes
//!   back exactly what the run did while nothing else changed since.
//! * Saved looks live in `looks.json` in the settings folder (next to `ui.json`): a JSON list of
//!   [`Look`]s, at most [`MAX_SAVED`], read with a size cap and made sane, so a bad file never
//!   crashes and never reaches the document unchecked.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_doc::{Block, Document, Pos};
use wordcraft_engine::assistant_edits::ScopeKind;
use wordcraft_engine::cowriter::{Output, Verb};
use wordcraft_engine::looks::{Look, MAX_SAVED, PRESETS};
use wordcraft_layout::DocLayout;

use crate::WordApp;
use crate::mac_controls as mc;
use crate::theme::Tokens;

/// The Before thumbnail's width, and its distance from the page.
pub const THUMB_W: f32 = 264.0;
const FROM_PAGE: f32 = 24.0;
/// How long Show's flash lasts.
const FLASH_SECS: f64 = 1.4;
/// The largest `looks.json` read.
const MAX_LOOKS_FILE: u64 = 256 * 1024;

/// What changed, in the list's order: (id the engine tags outputs with, label).
pub const WHAT: [(&str, &str); 9] = [
    ("look", "Look and styles"),
    ("headings", "Headings, title and subtitle"),
    ("lists", "Lists"),
    ("tables", "Tables"),
    ("fonts", "Fonts"),
    ("spacing", "Empty lines used as spacing"),
    ("typography", "Quotes, dashes and spaces"),
    ("headerFooter", "Header and page numbers"),
    ("numbering", "Numbered headings"),
];

/// The view of the document's last Format for me run.
pub struct FormatView {
    pub doc: u64,
    /// The look in use: a preset id or a saved look's name.
    pub look: String,
    /// The model's "Reads like a handbook" (empty without a model or when the look was given).
    pub guess: String,
    pub keep_colours: bool,
    pub toc: bool,
    pub number_headings: bool,
    pub scope: ScopeKind,
    /// (what id, how many) for the rows with something.
    pub what: Vec<(String, usize)>,
    /// From the identity check: (kept, changed).
    pub words: (usize, usize),
    pub checks: Vec<String>,
    /// The document before the run.
    pub before: Arc<Document>,
    before_layout: Option<Arc<DocLayout>>,
    /// Undo steps the view made, and the undo depth after the last one.
    pub steps: usize,
    pub depth: usize,
    /// Pictures of the old pages: (page, width in pixels) → texture.
    pictures: HashMap<(usize, u32), egui::TextureHandle>,
    flash: Option<(Vec<(Pos, Pos)>, f64)>,
    /// "Save as my look…" is asking for a name.
    naming: Option<String>,
    /// Show the old formatting as if ⌥ were held (the control channel, for screenshots).
    pub compare: bool,
}

impl FormatView {
    fn before_layout(&mut self) -> Option<Arc<DocLayout>> {
        if self.before_layout.is_none() {
            let s = wordcraft_engine::Session::new((*self.before).clone());
            self.before_layout = Some(s.export_layout());
        }
        self.before_layout.clone()
    }

    /// The old page `pi` as a texture `px` pixels wide.
    fn picture(&mut self, ctx: &egui::Context, pi: usize, px: u32) -> Option<egui::TextureHandle> {
        let px = px.clamp(64, 4096);
        if let Some(t) = self.pictures.get(&(pi, px)) {
            return Some(t.clone());
        }
        let layout = self.before_layout()?;
        let page = layout.pages.get(pi)?;
        if page.w <= 0.0 {
            return None;
        }
        let img = wordcraft_render::render_page(&self.before, page, px as f32 / page.w, &crate::canvas::screen_render_options());
        let rgba = img.to_straight();
        let tex = ctx.load_texture(
            format!("format-before-{pi}-{px}"),
            egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &rgba),
            egui::TextureOptions::LINEAR,
        );
        // A few sizes at most (zooming makes new ones).
        if self.pictures.len() > 12 {
            self.pictures.clear();
        }
        self.pictures.insert((pi, px), tex.clone());
        Some(tex)
    }
}

/// The view for the current document, if its last run was Format for me.
fn view(app: &mut WordApp) -> Option<&mut FormatView> {
    let doc = app.session.document_id();
    app.cowriter.format.as_mut().filter(|v| v.doc == doc)
}

pub fn active(app: &WordApp) -> bool {
    let doc = app.session.document_id();
    app.cowriter.format.as_ref().is_some_and(|v| v.doc == doc)
}

// ---------------------------------------------------------------------------------------------
// After a run

/// Record what a Format for me run did (`cowriter::land` calls it after `apply`).
#[allow(clippy::too_many_arguments)]
pub fn landed(
    app: &mut WordApp,
    outputs: &[Output],
    report: &str,
    checks: Vec<String>,
    words: Option<(usize, usize)>,
    before: Document,
    options: &Value,
    scope: ScopeKind,
) {
    let doc = app.session.document_id();
    let depth = app.session.undo_depth();
    let look = outputs
        .iter()
        .find_map(|o| match o {
            Output::FormatChange { params, .. } => params.get("look").and_then(Value::as_str).map(str::to_string),
            _ => None,
        })
        .unwrap_or_else(|| "clean".into());
    let (kept, changed) = words.unwrap_or((0, 0));
    let prev = app.cowriter.format.take().filter(|v| v.doc == doc);
    if changed > 0 {
        // Rule 5 stopped it: nothing changed, the summary says so.
        app.cowriter.format = prev;
        return;
    }
    let look_only = options.get("lookOnly").and_then(Value::as_bool).unwrap_or(false);
    if look_only && let Some(mut v) = prev {
        v.look = look;
        v.steps += 1;
        v.depth = depth;
        app.cowriter.format = Some(v);
        return;
    }
    let mut what: Vec<(String, usize)> = Vec::new();
    for o in outputs {
        if let Output::FormatChange { params, .. } = o {
            let id = params.get("what").and_then(Value::as_str).unwrap_or_default();
            let n = params.get("count").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
            match what.iter_mut().find(|w| w.0 == id) {
                Some(w) => w.1 = w.1.saturating_add(n),
                None => what.push((id.to_string(), n)),
            }
        }
    }
    what.retain(|w| w.1 > 0);
    what.sort_by_key(|w| WHAT.iter().position(|x| x.0 == w.0).unwrap_or(usize::MAX));
    let guess = report.split(" · ").next().filter(|g| !g.starts_with("Look: ")).unwrap_or_default().to_string();
    // A rerun (an option changed) keeps the first run's Before and counts on from it.
    let rerun = options.get("rerun").and_then(Value::as_bool).unwrap_or(false);
    let (before, steps, guess) = match prev {
        Some(p) if rerun => (p.before, p.steps + 1, if guess.is_empty() { p.guess } else { guess }),
        _ => (Arc::new(before), 1, guess),
    };
    let opt = |k: &str, d: bool| options.get(k).and_then(Value::as_bool).unwrap_or(d);
    let mut v = FormatView {
        doc,
        look,
        guess,
        keep_colours: opt("keepColours", true),
        toc: false,
        number_headings: opt("numberHeadings", false),
        scope,
        what,
        words: (kept, changed),
        checks,
        before,
        before_layout: None,
        steps,
        depth,
        pictures: HashMap::new(),
        flash: None,
        naming: None,
        compare: false,
    };
    if opt("toc", false) && add_toc(app) {
        v.toc = true;
        v.steps += 1;
        v.depth = app.session.undo_depth();
    }
    app.cowriter.format = Some(v);
}

/// T6 Add a table of contents: before the first heading, by the co-writer (tracked). It lists
/// the headings, so it runs after the identity check, as its own undo step.
fn add_toc(app: &mut WordApp) -> bool {
    let first_heading = app.session.doc.body.iter().position(|b| match &**b {
        Block::Para(p) => p.props.style.as_deref().is_some_and(|s| s.starts_with("Heading")),
        Block::Table(_) => false,
    });
    let keep = app.session.sel.clone();
    let at = Pos { story: wordcraft_doc::StoryRef::Body, path: wordcraft_doc::Path::top(first_heading.unwrap_or(0)), off: 0 };
    app.session.sel = wordcraft_engine::Selection::caret(at);
    let name = app.ui.assistant.name();
    let ok = app.session.run_as_assistant(&name, "references.toc", &json!({"levels": 3})).is_ok();
    app.session.sel = keep;
    app.session.clamp_selection();
    ok
}

// ---------------------------------------------------------------------------------------------
// Actions

enum Act {
    Look(String),
    Saved(usize),
    Option(&'static str, bool),
    UndoAll,
    OneByOne,
    Keep,
    Show(String),
    Name(Option<String>),
    SaveLook,
}

fn rerun(app: &mut WordApp, options: Value) {
    let Some(v) = app.cowriter.format.as_ref() else { return };
    let kind = v.scope;
    if let Err(e) = crate::cowriter::start(app, Verb::FormatForMe, kind, options) {
        app.status(e);
    }
}

fn act(app: &mut WordApp, a: Act) {
    match a {
        Act::Look(id) => rerun(app, json!({"look": id, "lookOnly": true})),
        Act::Saved(i) => {
            let Some(l) = saved_looks(app).get(i).cloned() else { return };
            rerun(app, json!({"look": l, "lookOnly": true}));
        }
        Act::Option(key, on) => {
            // Undo all and the reruns undo the view's own steps: only while nothing else changed.
            let depth = app.session.undo_depth();
            let Some(v) = view(app) else { return };
            let untouched = depth == v.depth;
            if key == "toc" {
                let was = v.toc;
                if on == was {
                    return;
                }
                let ok = if on {
                    add_toc(app)
                } else {
                    let name = app.ui.assistant.name();
                    app.session.run_as_assistant(&name, "references.removeToc", &json!({})).is_ok()
                };
                let depth = app.session.undo_depth();
                if let Some(v) = view(app)
                    && ok
                {
                    v.toc = on;
                    v.steps += 1;
                    v.depth = depth;
                }
                return;
            }
            // Keep my colours and Number the headings: the run again with the option changed.
            let (steps, look, keep, number, toc) = (v.steps, v.look.clone(), v.keep_colours, v.number_headings, v.toc);
            if !untouched {
                app.status(tl!("The document changed since it was formatted: run Format for me again instead."));
                return;
            }
            app.session.undo_steps(steps);
            let look_value = match PRESETS.contains(&look.as_str()) {
                true => json!(look),
                false => saved_looks(app).into_iter().find(|l| l.name == look).map_or(json!("clean"), |l| json!(l)),
            };
            let (keep, number) = if key == "keepColours" { (on, number) } else { (keep, on) };
            // The rerun keeps the first run's Before (`landed`, "rerun").
            if let Some(v) = app.cowriter.format.as_mut() {
                v.steps = 0;
            }
            rerun(app, json!({"look": look_value, "keepColours": keep, "numberHeadings": number, "toc": toc, "rerun": true}));
        }
        Act::UndoAll => {
            let depth = app.session.undo_depth();
            let Some(v) = view(app) else { return };
            let steps = v.steps;
            if depth != v.depth {
                app.status(tl!("The document changed since it was formatted: reject the co-writer’s changes in Review instead."));
                return;
            }
            app.session.undo_steps(steps);
            app.cowriter.format = None;
        }
        Act::OneByOne => {
            if !app.session.view.comments_pane {
                crate::mac_ui::toggle_panel(app, crate::mac_ui::Panel::Review);
            }
        }
        Act::Keep => {
            app.cowriter.format = None;
            if !crate::margin_notes::has_notes(app) {
                crate::mac_ui::set_margin(app, false);
            }
        }
        Act::Show(what) => show(app, &what),
        Act::Name(n) => {
            if let Some(v) = view(app) {
                v.naming = n;
            }
        }
        Act::SaveLook => save_look(app),
    }
}

/// T5 Show: select the first spot of `what` and flash them all.
fn show(app: &mut WordApp, what: &str) {
    let author = app.ui.assistant.name();
    let list = app.session.run("review.changes", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let has = |c: &Value, prop: &[&str]| c["props"].as_array().is_some_and(|p| p.iter().any(|x| prop.iter().any(|k| x["prop"] == *k)));
    let style_is = |c: &Value, f: &dyn Fn(&str) -> bool| {
        c["props"].as_array().is_some_and(|p| p.iter().any(|x| x["prop"] == "style" && x["value"].as_str().is_some_and(f)))
    };
    let mut spots: Vec<(Pos, Pos)> = list
        .iter()
        .filter(|c| c["author"].as_str().is_some_and(|a| crate::review_panel::is_assistant(a) || a == author))
        .filter(|c| match what {
            "headings" => c["kind"] == "format" && style_is(c, &|s| s.starts_with("Heading") || s == "Title" || s == "Subtitle"),
            "lists" => c["kind"] == "format" && has(c, &["numbering"]),
            "fonts" => c["kind"] == "format" && has(c, &["font", "color"]),
            "typography" | "spacing" => c["kind"] != "format",
            _ => false,
        })
        .filter_map(|c| Some((serde_json::from_value(c["start"].clone()).ok()?, serde_json::from_value(c["end"].clone()).ok()?)))
        .take(500)
        .collect();
    if what == "tables" {
        for (i, b) in app.session.doc.body.iter().enumerate() {
            if matches!(&**b, Block::Table(_)) {
                let at = |r: u32| Pos {
                    story: wordcraft_doc::StoryRef::Body,
                    path: wordcraft_doc::Path(vec![u32::try_from(i).unwrap_or(0), r, 0, 0]),
                    off: 0,
                };
                spots.push((at(0), at(0)));
            }
        }
    }
    let Some(first) = spots.first().cloned() else {
        app.status(tl!("Nothing to show on the page for that one."));
        return;
    };
    app.session.sel = wordcraft_engine::Selection { anchor: first.0, focus: first.1 };
    app.canvas.nav_to_caret = true;
    if let Some(v) = view(app) {
        v.flash = Some((spots, crate::now_ms() / 1000.0));
    }
}

/// `ui.assistant.formatView`: drive the view (`cowriter::command` lists the parameters), then
/// say what it shows.
pub fn control(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    if !active(app) {
        return Err("no Format for me run in this document".into());
    }
    if let Some(on) = p.get("compare").and_then(Value::as_bool)
        && let Some(v) = view(app)
    {
        v.compare = on;
    }
    if let Some(w) = p.get("show").and_then(Value::as_str) {
        show(app, w);
    }
    if let Some(o) = p.get("option").and_then(Value::as_str) {
        let key = match o {
            "keepColours" => "keepColours",
            "toc" => "toc",
            "numberHeadings" => "numberHeadings",
            _ => return Err("`option` must be keepColours, toc or numberHeadings".into()),
        };
        let on = p.get("value").and_then(Value::as_bool).ok_or("`value` must be true or false")?;
        act(app, Act::Option(key, on));
    }
    match p.get("action").and_then(Value::as_str) {
        None => {}
        Some("keep") => act(app, Act::Keep),
        Some("undoAll") => act(app, Act::UndoAll),
        Some("oneByOne") => act(app, Act::OneByOne),
        Some(_) => return Err("`action` must be keep, undoAll or oneByOne".into()),
    }
    let doc = app.session.document_id();
    Ok(match app.cowriter.format.as_ref().filter(|v| v.doc == doc) {
        Some(v) => json!({
            "open": true, "look": v.look, "guess": v.guess, "words": {"kept": v.words.0, "changed": v.words.1},
            "what": v.what.iter().map(|(w, n)| json!({"what": w, "count": n})).collect::<Vec<_>>(),
            "checks": v.checks, "keepColours": v.keep_colours, "toc": v.toc, "numberHeadings": v.number_headings, "steps": v.steps,
        }),
        None => json!({"open": false}),
    })
}

// ---------------------------------------------------------------------------------------------
// Saved looks

/// The saved looks (read from `looks.json` once a session; never more than [`MAX_SAVED`]).
pub fn saved_looks(app: &mut WordApp) -> Vec<Look> {
    if app.cowriter.saved_looks.is_none() {
        app.cowriter.saved_looks = Some(read_looks(app.cowriter.looks_file.as_deref()));
    }
    app.cowriter.saved_looks.clone().unwrap_or_default()
}

/// Read `path`: a JSON list of looks. Missing, too large or malformed: none.
pub fn read_looks(path: Option<&std::path::Path>) -> Vec<Look> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(p) = path else { return Vec::new() };
        if std::fs::metadata(p).map_or(true, |m| m.len() > MAX_LOOKS_FILE) {
            return Vec::new();
        }
        let Ok(bytes) = std::fs::read(p) else { return Vec::new() };
        parse_looks(&bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Vec::new()
    }
}

/// A JSON list of looks, each made sane; entries that aren't looks are skipped.
pub fn parse_looks(bytes: &[u8]) -> Vec<Look> {
    let Ok(Value::Array(list)) = serde_json::from_slice::<Value>(bytes) else { return Vec::new() };
    list.into_iter().filter_map(|v| serde_json::from_value::<Look>(v).ok()).filter(|l| !l.styles.is_empty()).map(Look::sane).take(MAX_SAVED).collect()
}

fn save_look(app: &mut WordApp) {
    let Some(name) = view(app).and_then(|v| v.naming.take()) else { return };
    let look = Look::from_doc(&app.session.doc, &name);
    let mut list = saved_looks(app);
    list.retain(|l| l.name != look.name);
    if list.len() >= MAX_SAVED {
        app.status(crate::i18n::fmt(
            tl!("You have {n} saved looks, the most there can be: remove one in looks.json first."),
            &[("n", &MAX_SAVED.to_string())],
        ));
        return;
    }
    list.push(look.clone());
    app.cowriter.saved_looks = Some(list.clone());
    if let Some(v) = view(app) {
        v.look = look.name.clone();
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(p) = app.cowriter.looks_file.clone() {
        let write = || -> std::io::Result<()> {
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)?;
            }
            let tmp = p.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(&list).unwrap_or_default())?;
            std::fs::rename(&tmp, &p)
        };
        if let Err(e) = write() {
            log::warn!("looks: {e}");
            app.status(tl!("The look is kept for this session only: it could not be saved."));
            return;
        }
    }
    app.status(crate::i18n::fmt(tl!("Saved “{name}” with your looks."), &[("name", &look.name)]));
}

// ---------------------------------------------------------------------------------------------
// Painting

/// The right margin's block at `at`, `w` wide. Returns its bottom.
pub fn paint_margin(app: &mut WordApp, ui: &mut Ui, at: Pos2, w: f32) -> f32 {
    let saved = saved_looks(app);
    let Some(v) = view(app) else { return at.y };
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let pad = 12.0;
    let rect = Rect::from_min_size(pos2(at.x + pad, at.y), vec2(w - pad, 4000.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)).id_salt("format_view"));
    let ui = &mut child;
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    let font = mc::font();
    let mut acts: Vec<Act> = Vec::new();
    ui.label(egui::RichText::new(tl!("Formatted by your co-writer")).font(font.clone()).color(m.assistant));
    // The words row: a lock, kept and changed.
    let words = crate::i18n::fmt(
        tl!("Your words: {changed} changed · {kept} kept"),
        &[("changed", &v.words.1.to_string()), ("kept", &v.words.0.to_string())],
    );
    let (row, _) = ui.allocate_exact_size(vec2(w - pad, 26.0), Sense::hover());
    ui.painter().rect_filled(row, 5.0, t.checked);
    crate::icons::paint(ui.painter(), Rect::from_min_size(row.min + vec2(6.0, 5.0), vec2(16.0, 16.0)), "lock", t.text, t.text);
    ui.painter().text(row.min + vec2(28.0, 13.0), Align2::LEFT_CENTER, words, font.clone(), t.text);
    // T4 Look.
    ui.add_space(4.0);
    let look_line = if v.guess.is_empty() { tl!("Look").to_string() } else { format!("{} · {}", tl!("Look"), v.guess) };
    ui.label(egui::RichText::new(look_line).font(font.clone()).color(t.text_dim));
    let segs: Vec<mc::Seg> = PRESETS.iter().map(|p| mc::Seg::Text(Look::label(p))).collect();
    let cur = PRESETS.iter().position(|p| *p == v.look);
    if let Some(i) = mc::segmented_one(ui, "format_look", &segs, cur)
        && let Some(id) = PRESETS.get(i)
        && cur != Some(i)
    {
        acts.push(Act::Look((*id).to_string()));
    }
    if !saved.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (i, l) in saved.iter().enumerate() {
                let on = l.name == v.look;
                let text = egui::RichText::new(&l.name).font(font.clone()).color(if on { t.text } else { t.accent });
                let r = ui.add(egui::Label::new(if on { text.underline() } else { text }).sense(Sense::click()));
                if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !on {
                    acts.push(Act::Saved(i));
                }
            }
        });
    }
    match &mut v.naming {
        None => {
            if mc::link(ui, "Save as my look…").clicked() {
                acts.push(Act::Name(Some(String::new())));
            }
        }
        Some(name) => {
            let r = ui.add(
                egui::TextEdit::singleline(name)
                    .hint_text(tl!("Name of the look"))
                    .desired_width(w - pad)
                    .char_limit(wordcraft_engine::looks::MAX_NAME),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if !r.has_focus() && name.is_empty() {
                r.request_focus();
            }
            let ok = !name.trim().is_empty();
            ui.horizontal(|ui| {
                if mc::button(ui, "Save", true, ok).clicked() || enter && ok {
                    acts.push(Act::SaveLook);
                }
                if mc::button(ui, "Cancel", false, true).clicked() {
                    acts.push(Act::Name(None));
                }
            });
        }
    }
    // T5 What changed.
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tl!("What changed")).font(font.clone()).color(t.text_dim));
    if v.what.is_empty() {
        ui.label(egui::RichText::new(tl!("Nothing needed formatting.")).font(font.clone()).color(t.text));
    }
    for (id, n) in &v.what {
        let label = WHAT.iter().find(|x| x.0 == id).map_or(id.as_str(), |x| x.1);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} · {n}", tl!(label))).font(font.clone()).color(t.text));
            if !matches!(id.as_str(), "look" | "headerFooter" | "numbering") {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if mc::link(ui, "Show").clicked() {
                        acts.push(Act::Show(id.clone()));
                    }
                });
            }
        });
    }
    // T6 Options.
    ui.add_space(4.0);
    let (keep, toc, number) = (v.keep_colours, v.toc, v.number_headings);
    // `checkbox` says whether it was clicked.
    if mc::checkbox(ui, "Keep my colours", keep) {
        acts.push(Act::Option("keepColours", !keep));
    }
    if mc::checkbox(ui, "Add a table of contents", toc) {
        acts.push(Act::Option("toc", !toc));
    }
    if mc::checkbox(ui, "Number the headings", number) {
        acts.push(Act::Option("numberHeadings", !number));
    }
    for c in &v.checks {
        ui.label(egui::RichText::new(c).font(font.clone()).color(t.text_dim));
    }
    // T7.
    ui.add_space(6.0);
    // Three buttons don't fit 256 pt: the two secondary ones are links, Keep the button.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        if mc::link(ui, "Undo all").clicked() {
            acts.push(Act::UndoAll);
        }
        if mc::link(ui, "One by one").clicked() {
            acts.push(Act::OneByOne);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if mc::button(ui, "Keep", true, true).clicked() {
                acts.push(Act::Keep);
            }
        });
    });
    let bottom = ui.min_rect().max.y;
    crate::review_panel::mark_line(ui.painter(), pos2(at.x + 1.0, at.y + 2.0), pos2(at.x + 1.0, bottom - 2.0), m.assistant, true);
    for a in acts {
        act(app, a);
    }
    bottom
}

/// T2 and Show's flash, beside and on the pages: the Before thumbnail in the left desk margin,
/// the old pages while ⌥ is held, the flashing spots.
pub fn paint_pages(app: &mut WordApp, ui: &mut Ui, painter: &egui::Painter, rects: &[Rect], layout: &DocLayout, scale: f32) {
    let hint = app.session.page_hint;
    let ctx = ui.ctx().clone();
    let clip = ui.clip_rect();
    let alt = (ui.input(|i| i.modifiers.alt) || app.cowriter.format.as_ref().is_some_and(|v| v.compare)) && !app.cowriter.working();
    let t = Tokens::get(&ctx);
    let m = crate::skin::OptimoTokens::get(&ctx);
    let ppp = ctx.pixels_per_point();
    // The page in view: the first one reaching the middle of the window.
    let mid = clip.center().y;
    let in_view = rects.iter().position(|r| r.max.y >= mid).or_else(|| rects.len().checked_sub(1)).unwrap_or(0);
    // Show's flash, before the view is borrowed.
    let mut flash_rects = Vec::new();
    let flash = view(app).and_then(|v| v.flash.clone());
    if let Some((spots, since)) = flash {
        let age = crate::now_ms() / 1000.0 - since;
        if age > FLASH_SECS {
            if let Some(v) = view(app) {
                v.flash = None;
            }
        } else {
            let alpha = (1.0 - age / FLASH_SECS).clamp(0.0, 1.0) as f32;
            for (a, b) in spots.iter().take(200) {
                let (a, b) = if a == b { (Pos { off: 0, ..a.clone() }, Pos { off: usize::MAX, ..b.clone() }) } else { (a.clone(), b.clone()) };
                for (pi, r) in layout.selection_rects(&app.session.doc, &a, &b.clone(), hint) {
                    if let (Some(pr), Some(page)) = (rects.get(pi), layout.pages.get(pi)) {
                        let s = crate::canvas::page_screen_scale(*pr, page, scale);
                        flash_rects.push((Rect::from_min_size(pos2(pr.min.x + r.x * s, pr.min.y + r.y * s), vec2(r.w * s, r.h * s)), alpha));
                    }
                }
            }
            ctx.request_repaint();
        }
    }
    let Some(v) = view(app) else { return };
    if alt {
        for (pi, pr) in rects.iter().enumerate() {
            if !pr.intersects(clip) {
                continue;
            }
            let px = (pr.width() * ppp).round().max(1.0) as u32;
            if let Some(tex) = v.picture(&ctx, pi, px) {
                painter.rect_filled(*pr, 0.0, Color32::WHITE);
                painter.image(tex.id(), *pr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        if let Some(pr) = rects.first() {
            let tag = tl!("Before — release ⌥ to see the formatting");
            painter.text(pos2(pr.center().x, clip.min.y + 14.0), Align2::CENTER_TOP, tag, mc::font(), m.assistant);
        }
    }
    for (r, a) in flash_rects {
        painter.rect_filled(r, 2.0, m.assistant.gamma_multiply(0.28 * a));
    }
    // The Before thumbnail, left of the page in view: 264 wide, smaller on a narrow desk, none
    // when there is no room for a readable one.
    let Some(pr) = rects.get(in_view).or(rects.first()) else { return };
    let room = pr.min.x - FROM_PAGE - clip.min.x - 12.0;
    if room < 120.0 {
        return;
    }
    let thumb_w = room.min(THUMB_W);
    let x = pr.min.x - FROM_PAGE - thumb_w;
    let Some(page) = v.before_layout().and_then(|l| l.pages.get(in_view.min(l.pages.len().saturating_sub(1))).cloned()) else { return };
    let h = thumb_w * page.h / page.w.max(1.0);
    let top = clip.min.y + 48.0;
    painter.text(pos2(x, top), Align2::LEFT_TOP, tl!("Before"), mc::font(), m.assistant);
    let img = Rect::from_min_size(pos2(x, top + 20.0), vec2(thumb_w, h));
    let thumb = in_view.min(v.before_layout().map_or(0, |l| l.pages.len().saturating_sub(1)));
    if let Some(tex) = v.picture(&ctx, thumb, (thumb_w * ppp).round() as u32) {
        painter.rect_filled(img, 0.0, Color32::WHITE);
        painter.image(tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    painter.rect_stroke(img, 0.0, Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
    painter.text(pos2(x, img.max.y + 8.0), Align2::LEFT_TOP, tl!("hold ⌥ to compare"), mc::font(), t.text_dim);
}

#[cfg(test)]
mod tests;
