//! O8 (OPTIMO.md §9 F1–F2, rule 4): Research in the right desk margin.
//!
//! * **F1** ⌘Y "Research…" or ⇧⌥⌘F opens the query field, prefilled with the selection, and a
//!   scope line in `text_dim` ("Web · your folders"): the person says what to look up; nothing
//!   is sent before Look up.
//! * **F2** Findings: one sentence in the source's own terms, the source as title · site · year
//!   in `text_dim` (a click opens the page), and Add as footnote · Add as quote · Dismiss. Nothing lands until a
//!   click; then one tracked change by the AI author, the source written in it.
//! * Web sources come from the provider's own search (Mistral's `web_search`, the same key);
//!   with another provider only the allowed files are searched, and the scope line says "Web
//!   sources need a search provider: Settings › Co-writer".

use egui::{Pos2, Rect, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::assistant_edits::{self as edits, ScopeKind};
use wordcraft_engine::cowriter::{self, Output, Ran, Verb};
use wordcraft_engine::research::{self as research, Page, Piece};

use crate::WordApp;
use crate::cowriter::Status;
use crate::mac_controls as mc;
use crate::theme::Tokens;

/// A finding and what became of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// A `Footnote` output (the form it was read in; Add as quote makes the `Quote`).
    pub output: Output,
    /// "footnote" / "quote" once added.
    pub added: Option<&'static str>,
}

/// The research view of one document.
pub struct ResearchView {
    pub doc: u64,
    pub query: String,
    pub findings: Vec<Finding>,
    /// The field takes the keyboard on the next frame.
    focus: bool,
}

enum Act {
    LookUp,
    Add(usize, bool),
    Open(String),
    Dismiss(usize),
    Close,
}

fn web(app: &WordApp) -> bool {
    app.ui.assistant.provider_id().web_search()
}

/// Where the findings may come from, for the scope line.
fn scope_line(app: &WordApp) -> String {
    let files = !app.ui.assistant.allowed_folders().is_empty();
    match (web(app), files) {
        (true, true) => tl!("Web · your folders").to_string(),
        (true, false) => tl!("Web").to_string(),
        (false, true) => format!("{} · {}", tl!("Your folders"), tl!("Web sources need a search provider: Settings › Co-writer")),
        (false, false) => tl!("Web sources need a search provider: Settings › Co-writer").to_string(),
    }
}

fn open_view(app: &mut WordApp, query: String) {
    let doc = app.session.document_id();
    app.cowriter.research = Some(ResearchView { doc, query, findings: Vec::new(), focus: true });
    app.cowriter.hide_summary = true;
    crate::mac_ui::set_margin(app, true);
}

/// Before `cowriter::start` runs Research: without a query the field opens instead (prefilled
/// from the selection); with one, the options get the sources and where a footnote goes. The
/// caller can't pass folders or switch the web on.
pub fn before_start(app: &mut WordApp, verb: Verb, _kind: ScopeKind, options: &mut Value) -> Option<Result<Value, String>> {
    if verb != Verb::Research {
        return None;
    }
    let query = options.get("query").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string();
    if query.is_empty() {
        let doc = app.session.document_id();
        let kept = app.cowriter.research.as_ref().filter(|v| v.doc == doc).map(|v| v.query.clone()).filter(|q| !q.trim().is_empty());
        let selected: String = if app.session.sel.is_collapsed() {
            String::new()
        } else {
            edits::scope(&app.session, ScopeKind::Selection).map(|s| s.text.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default()
        };
        let prefill = if selected.is_empty() { kept.unwrap_or_default() } else { selected.chars().take(200).collect() };
        open_view(app, prefill);
        return Some(Ok(json!({"asking": true})));
    }
    let anchor = research::anchor(&app.session).unwrap_or_default();
    let m = json!({
        "query": query,
        "folders": app.ui.assistant.allowed_folders(),
        "web": web(app),
        "anchor": anchor,
    });
    *options = m;
    let doc = app.session.document_id();
    match app.cowriter.research.as_mut().filter(|v| v.doc == doc) {
        Some(v) => {
            v.query = query;
            v.findings.clear();
        }
        None => open_view(app, query),
    }
    app.cowriter.hide_summary = false;
    None
}

/// Whether the run should go through the provider's web search.
pub fn uses_web(verb: Verb, options: &Value) -> bool {
    verb == Verb::Research && options.get("web").and_then(Value::as_bool) == Some(true)
}

/// The provider's web answer as the engine reads it.
#[cfg(not(target_arch = "wasm32"))]
pub fn envelope(w: &wordcraft_assistant::WebReply) -> String {
    let page = |p: &wordcraft_assistant::WebPage| Page { title: p.title.clone(), url: p.url.clone(), date: p.date.clone() };
    let pieces: Vec<Piece> = w
        .pieces
        .iter()
        .map(|p| match p {
            wordcraft_assistant::WebPiece::Text(t) => Piece::Text(t.clone()),
            wordcraft_assistant::WebPiece::Ref(r) => Piece::Cite(page(r)),
        })
        .collect();
    let searched: Vec<Page> = w.searched.iter().map(page).collect();
    research::envelope(&pieces, &searched)
}

/// The run's answer (`cowriter::land` hands it over): the findings are kept in the view, not
/// landed; the report and refusals stay with the run.
pub fn take(app: &mut WordApp, mut ran: Ran) -> Ran {
    let doc = app.session.document_id();
    let kept: Vec<Output> = std::mem::take(&mut ran.outputs);
    match app.cowriter.research.as_mut().filter(|v| v.doc == doc) {
        Some(v) => {
            for o in kept {
                match o {
                    o @ Output::Footnote { .. } => v.findings.push(Finding { output: o, added: None }),
                    // Notes (none today) land as they are.
                    o @ Output::Comment { .. } => ran.outputs.push(o),
                    _ => {}
                }
            }
        }
        None => ran.outputs.extend(kept.into_iter().filter(|o| matches!(o, Output::Comment { .. }))),
    }
    ran
}

pub fn active(app: &WordApp) -> bool {
    let doc = app.session.document_id();
    app.cowriter.research.as_ref().is_some_and(|v| v.doc == doc)
}

fn last_run(app: &WordApp) -> Option<&crate::cowriter::Run> {
    app.cowriter.runs(app.session.document_id()).last().filter(|r| r.verb == Verb::Research)
}

/// F1–F2 in the right margin at `at`, `w` wide; returns the bottom.
pub fn paint_margin(app: &mut WordApp, ui: &mut Ui, at: Pos2, w: f32) -> f32 {
    let working = app.cowriter.working() && last_run(app).is_some_and(|r| r.status == Status::Working);
    let line = scope_line(app);
    let run = last_run(app).map(|r| (r.report.clone(), r.lines.clone(), r.status));
    let t = Tokens::get(ui.ctx());
    let m = crate::skin::OptimoTokens::get(ui.ctx());
    let font = mc::font();
    let pad = 12.0;
    let Some(v) = app.cowriter.research.as_mut() else { return at.y };
    let rect = Rect::from_min_size(pos2(at.x + pad, at.y), vec2(w - pad, 4000.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)).id_salt("research_view"));
    let ui = &mut child;
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    let mut acts: Vec<Act> = Vec::new();
    ui.label(egui::RichText::new(tl!("Research")).font(font.clone()).color(m.assistant));
    let r = ui.add(
        egui::TextEdit::singleline(&mut v.query)
            .hint_text(tl!("What should I look up?"))
            .font(font.clone())
            .desired_width(w - pad)
            .char_limit(edits::MAX_INSTRUCTION_CHARS),
    );
    if std::mem::take(&mut v.focus) {
        r.request_focus();
    }
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        acts.push(Act::LookUp);
    }
    ui.add(egui::Label::new(egui::RichText::new(&line).font(font.clone()).color(t.text_dim)).wrap());
    ui.horizontal(|ui| {
        if mc::button(ui, "Look up", true, !working && !v.query.trim().is_empty()).clicked() {
            acts.push(Act::LookUp);
        }
        if mc::link(ui, "Close").clicked() {
            acts.push(Act::Close);
        }
    });
    if working {
        ui.label(egui::RichText::new(tl!("Looking it up…")).font(font.clone()).color(t.text_dim));
    } else if let Some((report, lines, status)) = run.filter(|(r, l, _)| !r.trim().is_empty() || !l.is_empty()) {
        ui.add_space(4.0);
        if !report.trim().is_empty() {
            ui.add(egui::Label::new(egui::RichText::new(&report).font(font.clone()).color(t.text)).wrap());
        }
        // Refusals are dim; a failure is the reason it failed.
        let color = if status == Status::Failed { t.text } else { t.text_dim };
        for l in lines.iter().take(8) {
            ui.add(egui::Label::new(egui::RichText::new(l).font(font.clone()).color(color)).wrap());
        }
    }
    for (i, f) in v.findings.iter().enumerate() {
        let Output::Footnote { text, source, .. } = &f.output else { continue };
        ui.add_space(6.0);
        ui.add(egui::Label::new(egui::RichText::new(text).font(font.clone()).color(t.text)).wrap());
        let src = if source.detail.is_empty() { source.title.clone() } else { format!("{} · {}", source.title, source.detail) };
        // The source opens its page (the links row has no room for a fourth link at 268 pt).
        let r = ui.add(egui::Label::new(egui::RichText::new(&src).font(font.clone()).color(t.text_dim)).wrap().sense(egui::Sense::click()));
        if !source.url.is_empty() {
            let r = r.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(format!("{} {}", tl!("Open"), source.url));
            if r.hovered() {
                ui.painter().hline(r.rect.x_range(), r.rect.max.y, egui::Stroke::new(1.0, t.text_dim));
            }
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, src.as_str()));
            if r.clicked() {
                acts.push(Act::Open(source.url.clone()));
            }
        }
        match f.added {
            Some(how) => {
                let said = if how == "quote" { tl!("Added as a quote") } else { tl!("Added as a footnote") };
                ui.label(egui::RichText::new(said).font(font.clone()).color(t.text_dim));
            }
            None => {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    if mc::link(ui, "Add as footnote").clicked() {
                        acts.push(Act::Add(i, false));
                    }
                    if mc::link(ui, "Add as quote").clicked() {
                        acts.push(Act::Add(i, true));
                    }
                    if mc::link(ui, "Dismiss").clicked() {
                        acts.push(Act::Dismiss(i));
                    }
                });
            }
        }
    }
    let bottom = ui.min_rect().max.y;
    crate::review_panel::mark_line(ui.painter(), pos2(at.x + 1.0, at.y + 2.0), pos2(at.x + 1.0, bottom), m.assistant, true);
    for a in acts {
        act(app, ui.ctx(), a);
    }
    bottom + 6.0
}

/// Start the run for the field's query.
fn look_up(app: &mut WordApp) {
    let Some(q) = app.cowriter.research.as_ref().map(|v| v.query.trim().to_string()).filter(|q| !q.is_empty()) else { return };
    let kind = if app.session.sel.is_collapsed() { ScopeKind::Section } else { ScopeKind::Selection };
    if let Err(e) = crate::cowriter::start(app, Verb::Research, kind, json!({"query": q})) {
        app.status(e);
    }
}

/// Land finding `i` as a footnote, or as a quote after the anchor's paragraph.
fn add(app: &mut WordApp, i: usize, quote: bool) -> Result<(), String> {
    let f = app.cowriter.research.as_ref().and_then(|v| v.findings.get(i)).filter(|f| f.added.is_none()).ok_or("That finding is gone.")?;
    let Output::Footnote { anchor, text, source } = f.output.clone() else { return Err("That finding is gone.".into()) };
    // The place it was for, else where the caret is now.
    let scope = edits::scope(&app.session, ScopeKind::Document)?;
    let anchor = if !anchor.is_empty() && edits::locate(&app.session, &scope, &anchor).is_ok() {
        anchor
    } else {
        research::anchor(&app.session).ok_or("Put the caret in the paragraph the finding is for.")?
    };
    let o = if quote { Output::Quote { after: anchor, text, source } } else { Output::Footnote { anchor, text, source } };
    let name = app.ui.assistant.name();
    crate::assistant_link::sync_policy(app);
    let landed = cowriter::apply(&mut app.session, &name, Verb::Research, &scope, vec![o]);
    if landed.applied.is_empty() {
        return Err(landed.lines.first().cloned().unwrap_or_else(|| tl!("Couldn’t add it.").into()));
    }
    app.ui.assistant.remember_name(&name);
    if let Some(f) = app.cowriter.research.as_mut().and_then(|v| v.findings.get_mut(i)) {
        f.added = Some(if quote { "quote" } else { "footnote" });
    }
    app.canvas.want_focus = true;
    Ok(())
}

fn act(app: &mut WordApp, ctx: &egui::Context, a: Act) {
    match a {
        Act::LookUp => look_up(app),
        Act::Add(i, quote) => {
            if let Err(e) = add(app, i, quote) {
                app.status(e);
            }
        }
        Act::Open(url) => {
            if research::web_address(&url).is_some() {
                ctx.open_url(egui::OpenUrl::new_tab(url));
            }
        }
        Act::Dismiss(i) => {
            if let Some(v) = app.cowriter.research.as_mut()
                && i < v.findings.len()
            {
                v.findings.remove(i);
            }
        }
        Act::Close => app.cowriter.research = None,
    }
}

/// `ui.assistant.researchView` (tests and screenshots, no model): `{"open": true, "query"?}`
/// opens the field; `{"answer": text, "query": q, "pages"?: [{title, url, date}]}` reads `text`
/// as the model's answer through the real reader and guards — with `pages`, a web answer whose
/// `⟦n⟧` marks a reference to `pages[n]` (all of them count as returned by the search);
/// `{"add": i, "as"?: "footnote" | "quote"}`, `{"dismiss": i}`, `{"close": true}` click as the
/// person would. Returns the view's state.
pub fn control(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    if p.get("open").and_then(Value::as_bool) == Some(true) {
        open_view(app, p.get("query").and_then(Value::as_str).unwrap_or_default().to_string());
    }
    if let Some(answer) = p.get("answer").and_then(Value::as_str) {
        let q = p.get("query").and_then(Value::as_str).unwrap_or("research").to_string();
        let mut options = json!({"query": q});
        before_start(app, Verb::Research, ScopeKind::Section, &mut options);
        let text = match p.get("pages").and_then(Value::as_array) {
            Some(list) => {
                let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let pages: Vec<Page> = list.iter().map(|v| Page { title: s(v, "title"), url: s(v, "url"), date: s(v, "date") }).collect();
                if let Some(m) = options.as_object_mut() {
                    m.insert("web".into(), json!(true));
                }
                research::envelope(&scripted(answer, &pages), &pages)
            }
            None => answer.to_string(),
        };
        crate::cowriter::run_with_options(app, Verb::Research, ScopeKind::Section, &text, &options)?;
    }
    if !active(app) {
        return Err("Research is not open in this document".into());
    }
    if let Some(i) = p.get("add").and_then(Value::as_u64) {
        add(app, usize::try_from(i).unwrap_or(usize::MAX), p.get("as").and_then(Value::as_str) == Some("quote"))?;
    }
    if let Some(i) = p.get("dismiss").and_then(Value::as_u64)
        && let Some(v) = app.cowriter.research.as_mut()
        && let Ok(i) = usize::try_from(i)
        && i < v.findings.len()
    {
        v.findings.remove(i);
    }
    if p.get("close").and_then(Value::as_bool) == Some(true) {
        app.cowriter.research = None;
    }
    Ok(match app.cowriter.research.as_ref() {
        Some(v) => json!({
            "open": true,
            "query": v.query,
            "findings": v.findings.iter().filter_map(|f| match &f.output {
                Output::Footnote { text, source, .. } => Some(json!({"text": text, "title": source.title, "detail": source.detail, "url": source.url, "added": f.added})),
                _ => None,
            }).collect::<Vec<_>>(),
        }),
        None => json!({"open": false}),
    })
}

/// `text` with `⟦n⟧` marks as answer pieces citing `pages[n]`.
fn scripted(text: &str, pages: &[Page]) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find('⟦') {
        out.push(Piece::Text(rest.get(..i).unwrap_or_default().to_string()));
        let after = rest.get(i + '⟦'.len_utf8()..).unwrap_or_default();
        let Some(j) = after.find('⟧') else { break };
        if let Some(p) = after.get(..j).and_then(|n| n.parse::<usize>().ok()).and_then(|n| pages.get(n)) {
            out.push(Piece::Cite(p.clone()));
        }
        rest = after.get(j + '⟧'.len_utf8()..).unwrap_or_default();
    }
    out.push(Piece::Text(rest.to_string()));
    out
}

#[cfg(test)]
mod tests {
    use wordcraft_engine::Session;

    use super::*;
    use crate::Services;

    fn app() -> WordApp {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("text.insert", &json!({"text": "The Albula line crosses the Alps.", "raw": true})).unwrap();
        WordApp::new(s, Services::default())
    }

    #[test]
    fn research_without_a_query_opens_the_field_prefilled_from_the_selection() {
        let mut a = app();
        a.session.run("select.text", &json!({"text": "Albula line"})).unwrap();
        let mut o = json!({});
        assert!(before_start(&mut a, Verb::Research, ScopeKind::Selection, &mut o).is_some());
        assert!(active(&a) && a.cowriter.margin);
        assert_eq!(a.cowriter.research.as_ref().unwrap().query, "Albula line");
        // The caller's folders and web switch are replaced by the person's settings.
        let mut o = json!({"query": "opening", "folders": ["/"], "web": true});
        assert!(before_start(&mut a, Verb::Research, ScopeKind::Selection, &mut o).is_none());
        assert_eq!(o["folders"], json!([]));
        assert_eq!(o["web"], json!(a.ui.assistant.provider_id().web_search()));
        assert_eq!(o["anchor"], json!("Albula line"));
    }

    #[test]
    fn a_scripted_web_answer_lists_findings_and_adds_one_as_a_footnote() {
        let mut a = app();
        let pages = json!([{"title": "Albula railway line", "url": "https://en.wikipedia.org/wiki/Albula_railway_line", "date": "2024"}]);
        let st = control(
            &mut a,
            &json!({"query": "opening", "answer": "Summary: One page.\n- It opened on 1 July 1903⟦0⟧.\n- From memory.", "pages": pages}),
        )
        .unwrap();
        assert_eq!(st["findings"].as_array().map(Vec::len), Some(1), "{st:#}");
        assert_eq!(st["findings"][0]["detail"], "en.wikipedia.org · 2024");
        // Nothing landed yet.
        assert!(a.session.run("review.changes", &json!({})).unwrap().as_array().unwrap().is_empty());
        let st = control(&mut a, &json!({"add": 0})).unwrap();
        assert_eq!(st["findings"][0]["added"], "footnote");
        let list = a.session.run("review.changes", &json!({})).unwrap();
        assert_eq!(list.as_array().map(Vec::len), Some(1), "{list:#}");
    }
}
