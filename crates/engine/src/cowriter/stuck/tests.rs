//! O9: reading the model's answer, pinning a question, the first line, and the helpers.

use serde_json::json;
use wordcraft_doc::{Document, Pos, StoryRef};

use super::super::*;
use crate::assistant_edits::{ScopeKind, scope};

const NAME: &str = "AI · Claude (co-writer)";

/// A session with a heading and a body line, the caret moved to a fresh empty paragraph.
fn stuck_session() -> (Session, Pos) {
    let mut s = Session::new(Document::new());
    s.author = "Martijn".into();
    s.run("text.insert", &json!({"text": "Why share a studio?", "raw": true})).unwrap();
    s.run("para.style", &json!({"style": "Heading1"})).unwrap();
    let at = crate::stuck::prepare(&mut s, None).unwrap();
    (s, at)
}

fn ran(s: &Session, answer: &str) -> Result<Ran, String> {
    let sc = crate::stuck::scope(s);
    let opts = json!({});
    run(Verb::Stuck, &Call { session: s, scope: &sc, options: &opts, name: NAME }, Some(Reply { text: answer, stop_reason: Some("stop") }))
}

fn text(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body)
}

#[test]
fn the_prompt_asks_for_questions_about_the_persons_own_knowledge() {
    let (s, _) = stuck_session();
    let sc = crate::stuck::scope(&s);
    let opts = json!({});
    let p = prompt(Verb::Stuck, &Call { session: &s, scope: &sc, options: &opts, name: NAME }).unwrap().unwrap();
    assert!(p.system.contains("only they can answer") && p.system.contains("never give answers") && p.system.contains("untrusted"));
    assert!(p.user.contains("Why share a studio?"));
}

#[test]
fn questions_and_one_first_line_are_read() {
    let (s, _) = stuck_session();
    let a = json!({"report": "A heading and nothing else.", "questions": ["Who first asked for this?", "What did it cost you last year?", "What would a visitor notice first?"],
        "first_line": "We started sharing the studio because the rent doubled."})
    .to_string();
    let r = ran(&s, &a).unwrap();
    assert_eq!(r.report, "A heading and nothing else.");
    let kinds: Vec<_> = r.outputs.iter().map(Output::kind).collect();
    assert_eq!(kinds, [OutputKind::Comment, OutputKind::Comment, OutputKind::Comment, OutputKind::FirstLine]);
    assert!(matches!(&r.outputs[0], Output::Comment { kind, .. } if kind == "Question"));
    // Fenced, and a bare list, are read too.
    assert_eq!(ran(&s, &format!("```json\n{a}\n```")).unwrap().outputs.len(), 4);
    assert_eq!(ran(&s, r#"["Who is it for?"]"#).unwrap().outputs.len(), 1);
}

#[test]
fn malformed_answers_are_refused_and_change_nothing() {
    let (s, _) = stuck_session();
    let before = text(&s);
    for bad in ["", "Sure! Here are some thoughts about studios.", r#"{"questions": ["Who?""#, "{{{{", "42"] {
        assert!(ran(&s, bad).is_err() || ran(&s, bad).unwrap().outputs.is_empty(), "{bad}");
    }
    assert_eq!(text(&s), before);
}

#[test]
fn prose_instead_of_questions_is_skipped_and_questions_are_capped() {
    let (s, _) = stuck_session();
    let many: Vec<String> = (1..=9).map(|i| format!("Question number {i}?")).collect();
    let a = json!({"questions": many.iter().cloned().chain(["Write about the rent.".to_string(), "Two lines?\nreally?".to_string()]).collect::<Vec<_>>()}).to_string();
    let r = ran(&s, &a).unwrap();
    assert_eq!(r.outputs.len(), crate::stuck::MAX_QUESTIONS);
    assert!(r.lines.iter().any(|l| l.contains("isn't a question")), "{:?}", r.lines);
    assert!(r.lines.iter().any(|l| l.contains("Kept the first")), "{:?}", r.lines);
    // Only prose: no questions at all, and it says so.
    let r = ran(&s, &json!({"report": "Hm.", "questions": ["The studio is nice."]}).to_string()).unwrap();
    assert!(r.outputs.is_empty() && r.lines.iter().any(|l| l.contains("no questions")));
}

#[test]
fn a_first_line_is_one_line_or_it_is_refused() {
    let (s, _) = stuck_session();
    let two_lines = json!({"questions": ["Who?"], "first_line": "First line.\nSecond line."}).to_string();
    let r = ran(&s, &two_lines).unwrap();
    assert_eq!(r.outputs.len(), 1, "the question stays, the first line is refused");
    assert!(r.lines.iter().any(|l| l.starts_with("Refused") && l.contains("rule 1")), "{:?}", r.lines);
    let two_sentences = json!({"questions": ["Who?"], "first_line": "We began in March. Then the rent doubled."}).to_string();
    assert_eq!(ran(&s, &two_sentences).unwrap().outputs.len(), 1);
    let long = format!("{{\"questions\": [\"Who?\"], \"first_line\": \"{}\"}}", ["word"; 41].join(" "));
    assert_eq!(ran(&s, &long).unwrap().outputs.len(), 1);
    // Nothing else is allowed (the §4 table): a replacement or footnote never passes.
    assert!(!Verb::Stuck.allows(OutputKind::Replacement) && !Verb::Stuck.allows(OutputKind::Footnote));
}

#[test]
fn prepare_uses_a_blank_paragraph_and_adds_one_otherwise() {
    let (mut s, at) = stuck_session();
    assert!(crate::stuck::still_here(&s, &at));
    assert_eq!(text(&s), "Why share a studio?\n");
    // Again from the blank paragraph: no second one.
    let again = crate::stuck::prepare(&mut s, None).unwrap();
    assert_eq!(again, at);
    assert_eq!(s.doc.para_paths(StoryRef::Body).len(), 2);
    // From a paragraph with text (O7 hands in the heading's path): a new one after it.
    let heading = s.doc.para_paths(StoryRef::Body)[0].clone();
    s.run("text.insert", &json!({"text": "x", "raw": true})).unwrap();
    let p = crate::stuck::prepare(&mut s, Some(&heading)).unwrap();
    assert!(crate::stuck::is_blank(&s, &p));
}

#[test]
fn clicking_a_question_pins_it_as_a_comment_by_the_ai_author_at_the_caret() {
    let (mut s, at) = stuck_session();
    let before = text(&s);
    let id = crate::stuck::pin(&mut s, NAME, &at, "Who first asked for this?").unwrap();
    let c = s.doc.comments.get(&id).unwrap();
    assert_eq!(c.author, NAME);
    assert_eq!(s.doc.plain_text(StoryRef::Part(c.part)).trim(), "Who first asked for this?");
    assert_eq!(text(&s).replace('\u{fffc}', ""), before.replace('\u{fffc}', ""), "no word was added");
    assert!(crate::stuck::still_here(&s, &at), "the paragraph still counts as empty, the caret stays");
    // A second question lands too; with comments by assistants off, none does.
    crate::stuck::pin(&mut s, NAME, &at, "What did it cost?").unwrap();
    s.assistant_comments = false;
    assert!(crate::stuck::pin(&mut s, NAME, &at, "Third?").is_err());
    assert!(crate::stuck::pin(&mut s, NAME, &at, "  ").is_err());
}

#[test]
fn the_first_line_lands_as_one_tracked_insert_by_the_ai_author() {
    let (mut s, at) = stuck_session();
    let sc = scope(&s, ScopeKind::Section).unwrap();
    let line = "We started sharing the studio because the rent doubled.";
    let o = vec![Output::FirstLine { at: "Why share a studio?".into(), text: line.into() }];
    let landed = apply(&mut s, NAME, Verb::Stuck, &sc, o);
    assert!(landed.lines.is_empty(), "{:?}", landed.lines);
    assert_eq!(landed.applied.len(), 1);
    assert_eq!(landed.applied[0].inserted, line);
    assert_eq!(text(&s), format!("Why share a studio?\n{line}"));
    let changes = s.run("review.changes", &json!({})).unwrap();
    let list = changes.as_array().unwrap();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0]["author"], NAME);
    assert_eq!(list[0]["kind"], "insert");
    // The paragraph is no longer empty: a second first line has nowhere to go.
    assert!(!crate::stuck::still_here(&s, &at));
    let again = apply(&mut s, NAME, Verb::Stuck, &sc, vec![Output::FirstLine { at: "x".into(), text: "Another line.".into() }]);
    assert!(again.applied.is_empty() && !again.lines.is_empty());
}

#[test]
fn from_your_notes_quotes_relevant_comments_with_their_origin() {
    let (mut s, _) = stuck_session();
    // A person's comment on the heading, one unrelated, and the co-writer's own.
    let h = s.doc.para_paths(StoryRef::Body)[0].clone();
    let on_heading = |s: &mut Session, who: &str, note: &str| {
        s.author = who.into();
        s.sel = crate::Selection {
            anchor: Pos { story: StoryRef::Body, path: h.clone(), off: 0 },
            focus: Pos { story: StoryRef::Body, path: h.clone(), off: 3 },
        };
        s.run("review.newComment", &json!({"text": note})).unwrap();
    };
    on_heading(&mut s, "Lena", "Mention the studio rent split from the minutes.");
    on_heading(&mut s, NAME, "Which studio?");
    let at = crate::stuck::prepare(&mut s, None).unwrap();
    let sc = crate::stuck::scope(&s);
    let notes = crate::stuck::from_notes(&s, &sc, NAME, &[]);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].text.contains("rent split") && notes[0].origin.starts_with("Comment by Lena"), "{notes:?}");
    assert!(crate::stuck::is_blank(&s, &at));
}
