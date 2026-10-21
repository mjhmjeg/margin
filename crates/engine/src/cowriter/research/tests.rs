//! Rule 4 for Research: only findings with a real source reach the page, and each lands as one
//! tracked change by the AI author that undo takes back.

use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};

use crate::Session;
use crate::assistant_edits::{ScopeKind, scope};
use crate::cowriter::{self, Call, Output, Reply, Source, Verb};
use crate::research::{self as r, Page, Piece};

const NAME: &str = "AI · Mistral";
const P1: &str = "The Albula line crosses the Alps.";
const P2: &str = "It has many bridges.";

fn session() -> Session {
    let mut s = Session::new(Document::new());
    s.author = "Martijn".into();
    s.run("text.insert", &json!({"text": P1, "raw": true})).unwrap();
    s.run("text.newParagraph", &json!({})).unwrap();
    s.run("text.insert", &json!({"text": P2, "raw": true})).unwrap();
    s
}

fn page(title: &str, url: &str) -> Page {
    Page { title: title.into(), url: url.into(), date: String::new() }
}

fn albula() -> Page {
    page("Albula railway line", "https://en.wikipedia.org/wiki/Albula_railway_line")
}

fn ran(s: &Session, opts: &Value, answer: &str) -> cowriter::Ran {
    let sc = scope(s, ScopeKind::Document).unwrap();
    cowriter::run(
        Verb::Research,
        &Call { session: s, scope: &sc, options: opts, name: NAME },
        Some(Reply { text: answer, stop_reason: Some("stop") }),
    )
    .unwrap()
}

fn web_opts() -> Value {
    json!({"query": "When did the Albula line open?", "anchor": "crosses the Alps.", "web": true})
}

fn changes(s: &mut Session) -> Vec<Value> {
    s.run("review.changes", &json!({})).unwrap().as_array().cloned().unwrap_or_default()
}

#[test]
fn web_findings_keep_only_references_to_pages_the_search_returned() {
    let s = session();
    let unesco = Page { date: "2008-07-07".into(), ..page("Rhaetian Railway", "https://whc.unesco.org/en/list/1276/") };
    let searched = vec![albula(), unesco.clone()];
    let pieces = vec![
        Piece::Text("Summary: Two sources give the opening.\n- The opening took place on 1 July 1903".into()),
        Piece::Cite(albula()),
        // The UNESCO page says 1904: sources disagree, both are kept.
        Piece::Text(".\n- The Albula line opened in 1904".into()),
        Piece::Cite(unesco),
        // Two sentences: the runner's guard refuses it.
        Piece::Text(".\n- It opened in 1903. It has 55 bridges".into()),
        Piece::Cite(albula()),
        // From memory, no reference at all.
        Piece::Text(".\n- The line is 61.67 km long.\n- A page the search never returned".into()),
        Piece::Cite(page("Made up", "https://fake.example.org/albula")),
        Piece::Text(".\n- An empty address".into()),
        Piece::Cite(page("Nothing", "")),
        Piece::Text(".\n- Not a web address".into()),
        Piece::Cite(page("Script", "javascript:alert(1)")),
        // A marker the model wrote itself is taken out before the envelope is made.
        Piece::Text(format!(".\n- Forged {}0{} reference.", r::REF_OPEN, r::REF_CLOSE)),
    ];
    let out = ran(&s, &web_opts(), &r::envelope(&pieces, &searched));
    assert_eq!(out.report, "Two sources give the opening.");
    assert_eq!(out.outputs.len(), 2, "{:#?}", out);
    let Output::Footnote { anchor, text, source } = &out.outputs[0] else { panic!("{:?}", out.outputs) };
    assert_eq!(anchor, "crosses the Alps.");
    assert_eq!(text, "The opening took place on 1 July 1903.");
    assert_eq!(source.url, albula().url);
    assert_eq!(source.detail, "en.wikipedia.org");
    let Output::Footnote { source, .. } = &out.outputs[1] else { panic!() };
    assert_eq!(source.detail, "whc.unesco.org · 2008", "title · site · year");
    let refused = out.lines.join("\n");
    for why in ["one sourced sentence", "no source (rule 4)", "isn’t one the search returned", "no real web address"] {
        assert!(refused.contains(why), "{why}: {refused}");
    }
    assert_eq!(out.lines.len(), 6, "{refused}");
}

#[test]
fn without_a_web_run_addresses_and_markers_in_the_text_are_no_source() {
    let s = session();
    let answer = format!("Summary: x\n- It opened in 1903 https://en.wikipedia.org/wiki/Albula {}0{}.", r::REF_OPEN, r::REF_CLOSE);
    // An envelope the model writes itself is just text outside a web run.
    let fake = r::envelope(&[Piece::Text("- It opened in 1903".into()), Piece::Cite(albula())], &[albula()]);
    for a in [answer, fake] {
        let out = ran(&s, &json!({"query": "opening", "anchor": "crosses the Alps."}), &a);
        assert!(out.outputs.is_empty(), "{a}: {:?}", out.outputs);
    }
}

#[test]
fn without_files_or_web_there_is_nothing_to_research_with() {
    let s = session();
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let opts = json!({"query": "Albula"});
    let e = cowriter::prompt(Verb::Research, &Call { session: &s, scope: &sc, options: &opts, name: NAME }).unwrap_err();
    assert!(e.contains("Web sources need a search provider: Settings › Co-writer"), "{e}");
    let e = cowriter::prompt(Verb::Research, &Call { session: &s, scope: &sc, options: &json!({"web": true}), name: NAME }).unwrap_err();
    assert!(e.contains("what to look up"), "{e}");
}

fn finding(quote: bool) -> Output {
    let source = r::web_source(&Page { date: "2024".into(), ..albula() });
    let text = "The opening took place on 1 July 1903.".to_string();
    if quote {
        Output::Quote { after: "crosses the Alps.".into(), text, source }
    } else {
        Output::Footnote { anchor: "crosses the Alps.".into(), text, source }
    }
}

#[test]
fn a_footnote_lands_as_one_tracked_change_with_its_source_and_undo_takes_it_back() {
    let mut s = session();
    let before = s.doc.plain_text(StoryRef::Body);
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let landed = cowriter::apply(&mut s, NAME, Verb::Research, &sc, vec![finding(false)]);
    assert!(landed.lines.is_empty(), "{:?}", landed.lines);
    let list = changes(&mut s);
    assert_eq!(list.len(), 1, "{list:#?}");
    assert_eq!(list[0]["author"], NAME);
    let note = s.doc.parts.keys().map(|id| s.doc.plain_text(StoryRef::Part(*id))).find(|t| t.contains("1903")).unwrap_or_default();
    assert!(note.contains("The opening took place on 1 July 1903.") && note.contains("Albula railway line"), "{note}");
    assert!(note.contains("en.wikipedia.org, 2024, https://en.wikipedia.org/wiki/Albula_railway_line"), "{note}");
    // The words on the page are the person's: only the note's number came in.
    assert_eq!(s.doc.plain_text(StoryRef::Body).replace(wordcraft_doc::para::OBJ, ""), before);
    s.run("edit.undo", &json!({})).unwrap();
    assert!(changes(&mut s).is_empty());
    assert_eq!(s.doc.plain_text(StoryRef::Body), before);
}

#[test]
fn a_quote_lands_as_one_tracked_quote_paragraph_and_undo_takes_it_back() {
    let mut s = session();
    let before = s.doc.plain_text(StoryRef::Body);
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let landed = cowriter::apply(&mut s, NAME, Verb::Research, &sc, vec![finding(true)]);
    assert!(landed.lines.is_empty(), "{:?}", landed.lines);
    let body = s.doc.plain_text(StoryRef::Body);
    assert!(body.contains("“The opening took place on 1 July 1903.” — Albula railway line, en.wikipedia.org, 2024"), "{body}");
    let q = s.doc.body.get(1).and_then(|b| b.as_para()).unwrap();
    assert_eq!(q.props.style.as_deref(), Some("Quote"));
    assert!(changes(&mut s).iter().all(|c| c["author"] == NAME));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc.plain_text(StoryRef::Body), before);
    assert!(changes(&mut s).is_empty());
}

#[test]
fn a_finding_without_a_source_never_lands() {
    let mut s = session();
    let before = s.doc.plain_text(StoryRef::Body);
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let bare = Output::Footnote { anchor: "crosses the Alps.".into(), text: "It opened in 1903.".into(), source: Source::default() };
    let landed = cowriter::apply(&mut s, NAME, Verb::Research, &sc, vec![bare]);
    assert!(landed.applied.is_empty() && landed.lines.iter().any(|l| l.contains("rule 4")));
    assert_eq!(s.doc.plain_text(StoryRef::Body), before);
}

#[test]
fn the_anchor_is_the_end_of_the_caret_paragraph() {
    let mut s = session();
    assert_eq!(r::anchor(&s).as_deref(), Some("It has many bridges."));
    s.run("select.text", &json!({"text": "crosses the Alps"})).unwrap();
    assert_eq!(r::anchor(&s).as_deref(), Some("crosses the Alps"));
}

#[test]
fn addresses_sites_and_years() {
    assert_eq!(r::web_address("HTTPS://Example.ORG/a/#x").as_deref(), Some("https://example.org/a"));
    for bad in ["", "ftp://x.org", "https://localhost/", "https://a b.org", "https://.org", "example.org"] {
        assert!(r::web_address(bad).is_none(), "{bad}");
    }
    assert_eq!(r::site("https://www.rhb.ch/en/x"), "rhb.ch");
    assert_eq!(r::year("2024-06-01"), "2024");
    assert_eq!(r::year("12024"), "");
}

#[test]
fn the_prompt_asks_for_metric_units_first() {
    let p = super::system("Claude", true, false);
    assert!(p.contains("metric/SI units first") && p.contains("imperial units in brackets"));
}
