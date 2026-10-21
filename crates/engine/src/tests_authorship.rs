//! Authorship of accepted assistant text (Optimo A5): accepting a suggestion marks it, rejecting
//! leaves nothing, the marks survive a `.docx` round trip and other formats drop them quietly.

use serde_json::json;
use wordcraft_doc::StoryRef;
use wordcraft_doc::authorship::{scan, span_text};

use crate::Session;

fn session(text: &str) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text(text));
    s.author = "Martijn".into();
    s.assistant_model = "claude-opus-5".into();
    s
}

fn run(s: &mut Session, id: &str, v: serde_json::Value) -> serde_json::Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn caret(s: &mut Session, block: usize, off: usize) {
    run(s, "select.range", json!({"anchor": {"block": block, "off": off}, "focus": {"block": block, "off": off}}));
}

/// The assistant inserts `text` at byte `off` of body paragraph `block`, as a tracked change.
fn suggest_insert(s: &mut Session, block: usize, off: usize, text: &str) {
    caret(s, block, off);
    s.run_as_assistant("Claude", "text.insert", &json!({"text": text, "raw": true})).unwrap();
}

/// The assistant replaces `quote` with `with` (tracked deletion + tracked insertion), as the
/// built-in assistant does.
fn suggest_replace(s: &mut Session, quote: &str, with: &str) {
    run(s, "select.text", json!({"text": quote}));
    let (a, b) = s.sel.ordered();
    s.run_as_assistant("Claude", "select.range", &json!({"anchor": a, "focus": b})).unwrap();
    s.run_as_assistant("Claude", "text.delete", &json!({})).unwrap();
    s.run_as_assistant("Claude", "select.range", &json!({"anchor": b, "focus": b})).unwrap();
    s.run_as_assistant("Claude", "text.insert", &json!({"text": with, "raw": true})).unwrap();
}

fn spans(s: &Session) -> Vec<String> {
    scan(&s.doc).ranges.iter().map(|r| span_text(&s.doc, r)).collect()
}

fn shares(s: &mut Session) -> (u64, u64, u64, u64) {
    let r = run(s, "authorship.shares", json!({}));
    (r["total"].as_u64().unwrap(), r["people"].as_u64().unwrap(), r["ai"].as_u64().unwrap(), r["pending"].as_u64().unwrap())
}

#[test]
fn accepting_an_assistant_insertion_marks_it_and_records_a_span() {
    let mut s = session("The cat sat on the mat.");
    suggest_insert(&mut s, 0, 11, " happily");
    assert_eq!(shares(&mut s), (31, 23, 0, 8), "pending text counts separately before the accept");
    caret(&mut s, 0, 13);
    run(&mut s, "review.accept", json!({}));
    assert_eq!(s.doc.plain_text(StoryRef::Body), "The cat sat happily on the mat.");
    assert!(run(&mut s, "review.changes", json!({})).as_array().unwrap().is_empty());
    assert_eq!(spans(&s), vec![" happily".to_string()]);
    let span = &s.doc.authorship.spans;
    assert_eq!(span.len(), 1);
    assert_eq!((span[0].author.as_str(), span[0].model.as_str(), span[0].text.as_deref()), ("Claude", "claude-opus-5", Some(" happily")));
    assert!(span[0].accepted_at.ends_with('Z') && span[0].accepted_at.contains('T'), "{}", span[0].accepted_at);
    assert!(span[0].name.starts_with("_OptimoAI_") && span[0].name.len() == "_OptimoAI_".len() + 8);
    assert_eq!(shares(&mut s), (31, 23, 8, 0));
    let list = run(&mut s, "authorship.spans", json!({}));
    assert_eq!(list[0]["chars"], 8);
    assert_eq!(list[0]["aiChars"], 8);
    assert_eq!(list[0]["edited"], false);
    assert_eq!(list[0]["author"], "Claude");
    // Undo takes the mark back with the accept.
    run(&mut s, "edit.undo", json!({}));
    assert!(s.doc.authorship.is_empty() && spans(&s).is_empty());
    assert_eq!(run(&mut s, "review.changes", json!({})).as_array().unwrap().len(), 1);
}

#[test]
fn rejecting_leaves_nothing_behind() {
    let mut s = session("The cat sat on the mat.");
    suggest_insert(&mut s, 0, 11, " happily");
    caret(&mut s, 0, 13);
    run(&mut s, "review.reject", json!({}));
    assert_eq!(s.doc.plain_text(StoryRef::Body), "The cat sat on the mat.");
    assert!(s.doc.authorship.is_empty() && spans(&s).is_empty());
    assert!(!wordcraft_docx::write(&s.doc).unwrap().windows(6).any(|w| w == b"margin"));
}

#[test]
fn accept_all_and_by_author_mark_only_the_assistants_text() {
    let mut s = session("One two three.");
    run(&mut s, "review.trackChanges", json!({"value": true}));
    caret(&mut s, 0, 3);
    run(&mut s, "text.insert", json!({"text": " (mine)", "raw": true}));
    suggest_insert(&mut s, 0, 0, "Start: ");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    assert_eq!(spans(&s), vec!["Start: ".to_string()]);
    assert_eq!(run(&mut s, "review.changes", json!({})).as_array().unwrap().len(), 1, "the person's change is still pending");
    run(&mut s, "review.acceptAll", json!({}));
    assert_eq!(spans(&s), vec!["Start: ".to_string()], "the person's accepted text gets no mark");
    assert_eq!(s.doc.plain_text(StoryRef::Body), "Start: One (mine) two three.");
    // A person named like nobody on the list is never an assistant; the matcher is one function.
    assert!(s.is_assistant_author(" claude ") && !s.is_assistant_author("Claudia") && !s.is_assistant_author(""));
}

#[test]
fn a_replacement_marks_only_the_inserted_text() {
    let mut s = session("The cat sat on the very big mat.");
    suggest_replace(&mut s, "very big", "large");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    assert_eq!(s.doc.plain_text(StoryRef::Body), "The cat sat on the large mat.");
    assert_eq!(spans(&s), vec!["large".to_string()]);
    assert_eq!(shares(&mut s), (29, 24, 5, 0));
}

#[test]
fn adjacent_and_nested_unedited_spans_merge_but_edited_ones_do_not() {
    let mut s = session("Alpha omega.");
    suggest_insert(&mut s, 0, 5, " beta");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    assert_eq!(spans(&s), vec![" beta".to_string()]);
    // Right after the span (the caret sits before its end marker): the insertion lands after
    // the bookmark end and, accepted, extends the unedited span.
    let end = scan(&s.doc).ranges[0].end.off;
    suggest_insert(&mut s, 0, end, " gamma");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    assert_eq!(spans(&s), vec![" beta gamma".to_string()]);
    assert_eq!(s.doc.authorship.spans.len(), 1);
    assert_eq!(s.doc.authorship.spans[0].text.as_deref(), Some(" beta gamma"));
    // Inside the span: nested, unedited → one span still.
    let start = scan(&s.doc).ranges[0].start.off + 3;
    suggest_insert(&mut s, 0, start + " beta".len(), "+");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    assert_eq!(spans(&s), vec![" beta+ gamma".to_string()]);
    assert_eq!(s.doc.authorship.spans.len(), 1);
    // The person edits inside; the next adjacent suggestion stays its own span.
    caret(&mut s, 0, start + 1);
    run(&mut s, "text.insert", json!({"text": "X", "raw": true}));
    let end = scan(&s.doc).ranges[0].end.off;
    suggest_insert(&mut s, 0, end, " delta");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    let mut got = spans(&s);
    got.sort();
    assert_eq!(got, vec![" Xbeta+ gamma".to_string(), " delta".to_string()]);
    assert_eq!(s.doc.authorship.spans.len(), 2);
    let r = run(&mut s, "authorship.shares", json!({}));
    assert_eq!((r["ai"].as_u64(), r["people"].as_u64()), (Some(18), Some(13)), "{r}");
    assert_eq!(s.doc.plain_text(StoryRef::Body), "Alpha Xbeta+ gamma delta omega.");
}

#[test]
fn outside_assistants_cannot_make_or_remove_marks() {
    let mut s = session("Hello world");
    run(&mut s, "select.text", json!({"text": "Hello"}));
    let e = s.run_as_assistant("Claude", "insert.bookmark", &json!({"name": "_OptimoAI_deadbeef"})).unwrap_err().to_string();
    assert!(e.starts_with("not allowed for assistants: insert.bookmark"), "{e}");
    let e = s.run_as_assistant("Claude", "insert.bookmark", &json!({"name": " _OptimoAI_x "})).unwrap_err().to_string();
    assert!(e.contains("insert.bookmark"), "{e}");
    // Margin's old prefix (still read as a mark) can't be forged either.
    let e = s.run_as_assistant("Claude", "insert.bookmark", &json!({"name": "_MarginAI_deadbeef"})).unwrap_err().to_string();
    assert!(e.starts_with("not allowed for assistants: insert.bookmark"), "{e}");
    s.run_as_assistant("Claude", "insert.bookmark", &json!({"name": "Intro"})).unwrap();
    assert!(scan(&s.doc).ranges.is_empty());
    // Read-only authorship commands stay open to them; the view switch too.
    s.run_as_assistant("Claude", "authorship.spans", &json!({})).unwrap();
    s.run_as_assistant("Claude", "authorship.shares", &json!({})).unwrap();
    // Every name an assistant has written under is on the list.
    s.run_as_assistant("AI · Mistral Large 4", "text.insert", &json!({"text": "x"})).unwrap();
    assert!(s.is_assistant_author("AI · Mistral Large 4"));
    assert_eq!(crate::authorship::display_name("AI · Mistral Large 4"), "Mistral Large 4");
    assert_eq!(crate::authorship::display_name("Claude"), "Claude");
}

#[test]
fn docx_keeps_the_marks_and_other_formats_drop_them_without_error() {
    let mut s = session("The cat sat on the mat.");
    suggest_insert(&mut s, 0, 11, " happily");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    let bytes = crate::io::save_bytes("a.docx", &s.doc).unwrap();
    let back = crate::io::open_bytes("a.docx", &bytes).unwrap();
    assert_eq!(back.authorship, s.doc.authorship);
    assert_eq!(scan(&back).ranges.len(), 1);
    assert_eq!(crate::io::save_bytes("a.docx", &back).unwrap(), bytes, "a second save is byte-stable");
    for name in ["a.odt", "a.md", "a.rtf", "a.html", "a.txt", "a.tex", "a.pdf"] {
        let out = crate::io::save_bytes(name, &s.doc).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!out.is_empty(), "{name}");
        assert!(!out.windows(10).any(|w| w == b"_OptimoAI_"), "{name}: the marks are stripped from copies in other formats");
        assert!(!out.windows(21).any(|w| w == b"urn:optimo:authorship"), "{name}: no store");
    }
    let dir = std::env::temp_dir().join(format!("wordcraft-authorship-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let md = dir.join("copy.md");
    let r = run(&mut s, "file.saveAs", json!({"path": md.to_string_lossy()}));
    assert_eq!(r["authorshipDropped"], true);
    assert!(s.status.contains("does not keep who wrote what"), "{}", s.status);
    let docx = dir.join("copy.docx");
    let r = run(&mut s, "file.saveAs", json!({"path": docx.to_string_lossy()}));
    assert!(r.get("authorshipDropped").is_none(), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
    // The session's own document keeps everything after the copies.
    assert_eq!(scan(&s.doc).ranges.len(), 1);
    assert_eq!(s.doc.authorship.spans.len(), 1);
}

#[test]
fn an_accepted_span_edited_by_a_person_is_shared_by_the_stored_text() {
    let mut s = session("Mist lifts off the river by eight.");
    suggest_replace(&mut s, "river", "lake");
    run(&mut s, "review.acceptAll", json!({"author": "Claude"}));
    assert_eq!(spans(&s), vec!["lake".to_string()]);
    // The person changes a letter inside: three of the four stay the AI's, the span is "edited".
    let start = scan(&s.doc).ranges[0].start.off + 3;
    run(&mut s, "select.range", json!({"anchor": {"block": 0, "off": start + 1}, "focus": {"block": 0, "off": start + 2}}));
    run(&mut s, "text.insert", json!({"text": "i", "raw": true}));
    assert_eq!(spans(&s), vec!["like".to_string()]);
    let list = run(&mut s, "authorship.spans", json!({}));
    assert_eq!((list[0]["chars"].as_u64(), list[0]["aiChars"].as_u64(), list[0]["edited"].as_bool()), (Some(4), Some(3), Some(true)));
    // Nothing of this is in the file but bookmarks and the store: no prompt, no path, no key.
    let bytes = crate::io::save_bytes("a.docx", &s.doc).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("BEGIN DOCUMENT") && !text.contains("sk-") && !text.contains("/Users/"));
}

#[test]
fn the_view_switch_is_a_command() {
    let mut s = session("x");
    assert!(!s.view.authorship);
    assert_eq!(run(&mut s, "view.authorship", json!({}))["value"], true);
    assert!(s.view.authorship);
    assert_eq!(run(&mut s, "view.authorship", json!({"value": false}))["value"], false);
    assert!(s.registry.get("view.authorship").is_some_and(|c| c.label == "Show Authorship"));
}

/// The "AI · " prefix makes an author an assistant on any computer (no names configured); an
/// older plain name counts only through the legacy list; a person whose name starts with "Ai"
/// never does.
#[test]
fn the_ai_prefix_and_legacy_names_decide_who_is_an_assistant() {
    use crate::authorship::{display_name, is_assistant_name, with_ai_prefix};
    let none: [&str; 0] = [];
    assert!(is_assistant_name("AI · Mistral Large 4", &none), "another Mac: no names known");
    assert!(!is_assistant_name("Mistral", &none));
    assert!(is_assistant_name("Mistral", &["Mistral"]), "legacy name");
    for person in ["Aiko", "AI Weiwei", "AI: x", "ai · lower", "Martijn"] {
        assert!(!is_assistant_name(person, &["Mistral", "Claude"]), "{person}");
    }
    assert_eq!(with_ai_prefix("Mistral Large 4"), "AI · Mistral Large 4");
    assert_eq!(with_ai_prefix("AI · Claude"), "AI · Claude", "never doubled");
    assert_eq!(display_name(" AI · Mistral Large 4 "), "Mistral Large 4");

    // Accepted text by the prefixed name and by the legacy name counts as AI; Aiko's doesn't.
    let mut s = session("One two three.");
    s.assistant_authors = vec!["Mistral".into()];
    for (who, off, text) in [("AI · Mistral Large 4", 3, " big"), ("Mistral", 7, " small")] {
        caret(&mut s, 0, off);
        s.run_as_assistant(who, "text.insert", &json!({"text": text, "raw": true})).unwrap();
    }
    // Aiko is a person: her suggestion is tracked under her name.
    s.author = "Aiko".into();
    run(&mut s, "review.trackChanges", json!({"value": true}));
    caret(&mut s, 0, 13 + " big".len() + " small".len());
    run(&mut s, "text.insert", json!({"text": " end", "raw": true}));
    run(&mut s, "review.trackChanges", json!({"value": false}));
    assert!(s.is_assistant_author("AI · Mistral Large 4") && s.is_assistant_author("Mistral") && !s.is_assistant_author("Aiko"));
    run(&mut s, "review.acceptAll", json!({}));
    let (_, _, ai, _) = shares(&mut s);
    assert_eq!(ai as usize, " big".len() + " small".len(), "Aiko's words are hers");
}
