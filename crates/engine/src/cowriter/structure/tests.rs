//! O7: reading the model's answer, the outline, and landing a move or a heading.

use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};

use super::super::*;
use crate::assistant_edits::{ScopeKind, scope};
use crate::structure as st;

const NAME: &str = "AI · Mistral";

/// Intro, then Rules (2 lines), Membership (1 line), Events (empty), Contact (1 line).
fn doc() -> Session {
    let mut s = Session::new(Document::new());
    s.author = "Martijn".into();
    let lines: [(&str, Option<&str>); 9] = [
        ("Our studio is shared by six people.", None),
        ("Rules", Some("Heading1")),
        ("Clean the press after use.", None),
        ("Book the big table a day ahead.", None),
        ("Membership", Some("Heading1")),
        ("Members pay monthly and get a key.", None),
        ("Events", Some("Heading1")),
        ("Contact", Some("Heading1")),
        ("Write to the studio address.", None),
    ];
    for (i, (t, style)) in lines.iter().enumerate() {
        if i > 0 {
            s.run("text.newParagraph", &json!({})).unwrap();
        }
        s.run("text.insert", &json!({"text": t, "raw": true})).unwrap();
        s.run("para.style", &json!({"style": style.unwrap_or("Normal")})).unwrap();
    }
    s
}

fn ran(s: &Session, answer: &str) -> Result<Ran, String> {
    let sc = scope(s, ScopeKind::Document).unwrap();
    let opts = json!({});
    run(Verb::Structure, &Call { session: s, scope: &sc, options: &opts, name: NAME }, Some(Reply { text: answer, stop_reason: Some("stop") }))
}

fn paras(s: &Session) -> Vec<String> {
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.final_text()).collect()
}

fn changes(s: &mut Session) -> Vec<(String, String)> {
    let ch = s.run("review.changes", &json!({})).unwrap();
    ch.as_array().unwrap().iter().map(|c| (c["kind"].as_str().unwrap_or("").to_string(), c["author"].as_str().unwrap_or("").to_string())).collect()
}

#[test]
fn the_outline_has_each_heading_with_its_words() {
    let s = doc();
    let o = st::outline(&s);
    let got: Vec<(&str, usize)> = o.iter().map(|x| (x.heading.as_str(), x.words)).collect();
    assert_eq!(got, [("Rules", 12), ("Membership", 7), ("Events", 0), ("Contact", 5)]);
    assert!(o[2].empty() && o[1].thin());
    assert_eq!(st::total_words(&s), 7 + 4 + 12 + 7 + 5);
    assert_eq!(st::find(&o, "  membership ").map(|x| x.block), Some(4));
}

#[test]
fn the_prompt_sends_the_outline_and_forbids_body_text() {
    let s = doc();
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let opts = json!({});
    let p = prompt(Verb::Structure, &Call { session: &s, scope: &sc, options: &opts, name: NAME }).unwrap().unwrap();
    assert!(p.user.contains("- Membership (7 words)") && p.user.contains("- Events (0 words)"));
    assert!(p.system.contains("Never write body text") && p.system.contains("untrusted"));
}

#[test]
fn malformed_answers_change_nothing() {
    let s = doc();
    for bad in ["", "Sure! Move Membership up.", "{\"moves\": [", "[1, 2]", "\"text\""] {
        assert!(ran(&s, bad).is_err(), "{bad:?}");
    }
    // Fenced and with a think block still reads.
    let r = ran(&s, "<think>hm</think>```json\n{\"report\": \"Fine.\"}\n```").unwrap();
    assert_eq!(r.report, "Fine.");
}

#[test]
fn moves_naming_headings_that_dont_exist_are_refused() {
    let s = doc();
    let r = ran(
        &s,
        r#"{"report": "r", "moves": [
            {"heading": "Membership", "before": "Rules", "why": "Readers decide to join first."},
            {"heading": "Pricing", "before": "Rules"},
            {"heading": "Rules", "before": "Nowhere"},
            {"heading": "Rules", "before": "Membership"}
        ]}"#,
    )
    .unwrap();
    assert_eq!(r.outputs, [Output::HeadingMove { heading: "Membership".into(), before: "Rules".into() }]);
    assert_eq!(r.lines.iter().filter(|l| l.contains("isn't in the document")).count(), 2, "{:?}", r.lines);
    assert!(r.lines.iter().any(|l| l.contains("already there")), "{:?}", r.lines);
}

#[test]
fn prose_headings_are_refused() {
    let s = doc();
    let r = ran(
        &s,
        r#"{"missing": [
            {"heading": "Costs", "after": "Membership", "why": "A reader will look for what membership costs."},
            {"heading": "This section should explain what membership costs.", "after": "Membership"},
            {"heading": "Costs and fees and dues and charges and what a member pays every single month", "after": "Membership"},
            {"heading": "Two\nlines", "after": "Membership"},
            {"heading": "Rules", "after": "Contact"},
            {"heading": "History", "after": "Founding"}
        ],
        "notes": [{"quote": "Members pay monthly", "note": "This section also explains keys."}, {"quote": "not there", "note": "x"}]}"#,
    )
    .unwrap();
    assert_eq!(
        r.outputs,
        [
            Output::HeadingInsert { after: "Membership".into(), text: "Costs".into(), level: 1 },
            Output::Comment {
                anchor: "Members pay monthly".into(),
                kind: st::KIND.into(),
                text: "This section also explains keys.".into(),
                options: vec![]
            },
        ]
    );
    assert_eq!(r.lines.iter().filter(|l| l.starts_with("Refused")).count(), 4, "{:?}", r.lines);
    // The guard holds on its own too.
    let (kept, refused) =
        guard(Verb::Structure, "", vec![Output::HeadingInsert { after: "x".into(), text: "a b c d e f g h i j k l m".into(), level: 1 }]);
    assert!(kept.is_empty() && refused.len() == 1);
}

#[test]
fn accepting_a_move_is_one_tracked_move_by_the_ai_that_keeps_every_word_and_undoes() {
    let mut s = doc();
    let before = paras(&s);
    let words = |s: &Session| {
        let mut w = plain_words(&s.doc.plain_text(StoryRef::Body));
        w.sort();
        w
    };
    let w0 = words(&s);
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let landed = apply(&mut s, NAME, Verb::Structure, &sc, vec![Output::HeadingMove { heading: "Membership".into(), before: "Rules".into() }]);
    assert!(landed.lines.is_empty(), "{:?}", landed.lines);
    assert_eq!(landed.applied.len(), 1);
    let ch = changes(&mut s);
    assert!(!ch.is_empty() && ch.iter().all(|(k, a)| k.starts_with("move") && a == NAME), "{ch:?}");
    // As it reads after accepting: Membership before Rules, nothing lost.
    let after = paras(&s);
    let order: Vec<&str> = after.iter().map(String::as_str).filter(|t| !t.is_empty()).collect();
    assert_eq!(order[1..5], ["Membership", "Members pay monthly and get a key.", "Rules", "Clean the press after use."]);
    assert_eq!(words(&s).len(), w0.len() + 8, "the moved words are held at both ends until accepted");
    // The person's tracking stays off; one Undo puts it all back.
    assert!(!s.doc.settings.track_changes);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(paras(&s), before);
    assert!(changes(&mut s).is_empty());
    // Accepted, the document holds the same words in the new order.
    let sc = scope(&s, ScopeKind::Document).unwrap();
    apply(&mut s, NAME, Verb::Structure, &sc, vec![Output::HeadingMove { heading: "Membership".into(), before: "Rules".into() }]);
    s.run("review.acceptAll", &json!({})).unwrap();
    assert_eq!(words(&s), w0);
    assert_eq!(st::outline(&s).iter().map(|x| x.heading.as_str()).collect::<Vec<_>>(), ["Membership", "Rules", "Events", "Contact"]);
}

#[test]
fn the_last_section_moves_too_and_rejecting_puts_it_back() {
    let mut s = doc();
    let before = paras(&s);
    st::move_section(&mut s, Some(NAME), "Contact", "Membership").unwrap();
    let heads: Vec<String> = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
    assert!(heads.iter().filter(|t| *t == "Contact").count() == 2, "both ends of the move: {heads:?}");
    s.run("review.acceptAll", &json!({})).unwrap();
    assert_eq!(
        st::outline(&s).iter().map(|x| (x.heading.as_str(), x.words)).collect::<Vec<_>>(),
        [("Rules", 12), ("Contact", 5), ("Membership", 7), ("Events", 0)]
    );
    let mut s = doc();
    st::move_section(&mut s, Some(NAME), "Contact", "Membership").unwrap();
    s.run("review.rejectAll", &json!({})).unwrap();
    assert_eq!(paras(&s), before);
    // A section can't go inside itself; names that aren't there change nothing.
    let mut s = doc();
    assert!(st::move_section(&mut s, Some(NAME), "Rules", "Nowhere").is_err());
    assert!(st::move_section(&mut s, Some(NAME), "Rules", "Membership").is_err());
    assert_eq!(paras(&s), before);
}

#[test]
fn accepting_a_missing_heading_is_one_tracked_heading_and_nothing_else() {
    let mut s = doc();
    let before = paras(&s);
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let landed =
        apply(&mut s, NAME, Verb::Structure, &sc, vec![Output::HeadingInsert { after: "Membership".into(), text: "Costs".into(), level: 1 }]);
    assert!(landed.lines.is_empty(), "{:?}", landed.lines);
    assert_eq!(st::outline(&s).iter().map(|x| x.heading.as_str()).collect::<Vec<_>>(), ["Rules", "Membership", "Costs", "Events", "Contact"]);
    let ch = changes(&mut s);
    assert!(ch.iter().any(|(k, _)| k == "insert") && ch.iter().all(|(_, a)| a == NAME), "{ch:?}");
    let costs = s.doc.body.iter().filter_map(|b| b.as_para()).find(|p| p.text == "Costs").unwrap();
    assert!(costs.runs.iter().all(|r| r.props.ins.is_some()), "the heading text is a tracked insert");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(paras(&s), before);
    // Prose and a second copy of a heading are refused when landing too.
    assert!(st::insert_heading(&mut s, NAME, "Rules", "Members pay monthly and get a key.", 1).is_err());
    assert!(st::insert_heading(&mut s, NAME, "Rules", "Contact", 1).is_err());
    assert_eq!(paras(&s), before);
}

#[test]
fn the_persons_clipboard_survives_a_move() {
    let mut s = doc();
    s.clipboard_text = "mine".into();
    st::move_section(&mut s, Some(NAME), "Membership", "Rules").unwrap();
    assert_eq!(s.clipboard_text, "mine");
    assert!(s.pending_move.is_none());
    let _: Value = s.run("review.changes", &json!({})).unwrap();
}

#[test]
fn a_section_moved_down_keeps_every_heading_a_heading() {
    let mut s = doc();
    st::move_section(&mut s, Some(NAME), "Rules", "Contact").unwrap();
    s.run("review.acceptAll", &json!({})).unwrap();
    assert_eq!(
        st::outline(&s).iter().map(|x| (x.heading.as_str(), x.words)).collect::<Vec<_>>(),
        [("Membership", 7), ("Events", 0), ("Rules", 12), ("Contact", 5)]
    );
}

#[test]
fn the_persons_drag_is_their_own_tracked_move() {
    let mut s = doc();
    st::move_section(&mut s, None, "Membership", "Rules").unwrap();
    let ch = changes(&mut s);
    assert!(!ch.is_empty() && ch.iter().all(|(k, a)| k.starts_with("move") && a == "Martijn"), "{ch:?}");
    assert!(!s.doc.settings.track_changes, "the person's own setting is back");
}
