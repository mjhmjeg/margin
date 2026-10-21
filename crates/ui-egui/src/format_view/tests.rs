//! Format for me's view in the app: no model needed, the look switcher and options as undo
//! steps the view counts, Undo all, saved looks on disk.

use serde_json::json;
use wordcraft_doc::StoryRef;
use wordcraft_engine::Session;

use super::*;
use crate::Services;

fn app_with(paras: &[&str]) -> WordApp {
    let mut a = WordApp::new(Session::new(Document::new()), Services::default());
    a.session.author = "Martijn".into();
    a.session.prefs.autocorrect.enabled = false;
    for (i, p) in paras.iter().enumerate() {
        if i > 0 {
            a.session.run("text.newParagraph", &json!({})).unwrap();
        }
        a.session.run("text.insert", &json!({"text": p, "raw": true})).unwrap();
    }
    a
}

const MESSY: [&str; 7] = ["Studio rules", "Spring update", "", "Don't leave \"wet\" pots...", "", "- Clay", "- Glaze"];

fn text(a: &WordApp) -> String {
    a.session.doc.plain_text(StoryRef::Body)
}

fn state(a: &mut WordApp) -> Value {
    a.run("ui.assistant.formatView", json!({})).unwrap()
}

#[test]
fn formatting_needs_no_model_and_undo_all_takes_it_back() {
    let mut a = app_with(&MESSY);
    let before = text(&a);
    assert!(!a.ui.assistant.enabled, "the co-writer is off: nothing may be sent");
    a.run("ui.assistant.run", json!({"verb": "format"})).unwrap();
    assert!(!a.cowriter.working(), "rule-based: it lands at once");
    let v = state(&mut a);
    assert_eq!(v["look"], "clean");
    assert_eq!(v["guess"], "");
    assert_eq!(v["words"]["changed"], 0);
    assert_eq!(v["words"]["kept"], 10);
    let what: Vec<&str> = v["what"].as_array().unwrap().iter().filter_map(|w| w["what"].as_str()).collect();
    for w in ["look", "headings", "lists", "spacing", "typography", "headerFooter"] {
        assert!(what.contains(&w), "{w}: {what:?}");
    }
    assert!(a.cowriter.margin, "the margin view opens");
    assert_ne!(text(&a), before);
    a.run("ui.assistant.formatView", json!({"action": "undoAll"})).unwrap();
    assert_eq!(text(&a), before);
    assert!(!active(&a));
    assert!(a.session.run("review.changes", &json!({})).unwrap().as_array().unwrap().is_empty());
}

#[test]
fn looks_and_options_are_steps_undo_all_counts() {
    let mut a = app_with(&MESSY);
    let before = text(&a);
    let styles = a.session.doc.styles.clone();
    a.run("ui.assistant.run", json!({"verb": "format"})).unwrap();
    a.run("ui.assistant.run", json!({"verb": "format", "look": "classic", "lookOnly": true})).unwrap();
    let v = state(&mut a);
    assert_eq!((v["look"].as_str(), v["steps"].as_u64()), (Some("classic"), Some(2)));
    let serif = |a: &WordApp| a.session.doc.styles.find("Heading 1").and_then(|s| s.chr.font.clone());
    assert_eq!(serif(&a).as_deref(), Some("Source Serif 4"));
    // An option reruns the format with the same look, keeping the first Before.
    a.run("ui.assistant.formatView", json!({"option": "keepColours", "value": false})).unwrap();
    let v = state(&mut a);
    assert_eq!((v["look"].as_str(), v["keepColours"].as_bool()), (Some("classic"), Some(false)));
    a.run("ui.assistant.formatView", json!({"option": "toc", "value": true})).unwrap();
    assert_eq!(state(&mut a)["toc"], true);
    a.run("ui.assistant.formatView", json!({"action": "undoAll"})).unwrap();
    assert_eq!(text(&a), before);
    assert_eq!(a.session.doc.styles.find("Heading 1").map(|s| s.chr.clone()), styles.find("Heading 1").map(|s| s.chr.clone()));
}

#[test]
fn undo_all_refuses_once_the_person_edited() {
    let mut a = app_with(&MESSY);
    a.run("ui.assistant.run", json!({"verb": "format"})).unwrap();
    a.session.run("text.insert", &json!({"text": "x"})).unwrap();
    a.run("ui.assistant.formatView", json!({"action": "undoAll"})).unwrap();
    assert!(active(&a), "nothing undone: the person's edit came after");
    assert!(text(&a).contains('x'));
    a.run("ui.assistant.formatView", json!({"action": "keep"})).unwrap();
    assert!(!active(&a));
}

#[test]
fn show_selects_the_spots() {
    let mut a = app_with(&MESSY);
    a.run("ui.assistant.run", json!({"verb": "format"})).unwrap();
    a.session.sel = wordcraft_engine::Selection::caret(a.session.doc.start_of(StoryRef::Body));
    a.run("ui.assistant.formatView", json!({"show": "lists"})).unwrap();
    let (s, _) = a.session.sel.ordered();
    assert_eq!(s.path, wordcraft_doc::Path::top(5), "the first list item");
    assert!(a.cowriter.format.as_ref().is_some_and(|v| v.flash.is_some()));
}

#[test]
fn saved_looks_are_bounded_and_round_trip() {
    assert!(parse_looks(b"not json").is_empty());
    assert!(parse_looks(b"{\"name\": \"x\"}").is_empty());
    assert!(parse_looks(b"[1, \"x\", {\"name\": 3}]").is_empty());
    let one = serde_json::to_vec(&vec![Look::preset("report").unwrap(); MAX_SAVED + 5]).unwrap();
    assert_eq!(parse_looks(&one).len(), MAX_SAVED);
    let dir = std::env::temp_dir().join(format!("optimo-looks-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let big = dir.join("big.json");
    std::fs::write(&big, vec![b' '; (MAX_LOOKS_FILE + 1) as usize]).unwrap();
    assert!(read_looks(Some(&big)).is_empty(), "too large: not read");
    assert!(read_looks(Some(&dir.join("missing.json"))).is_empty());
    // Save as my look… writes looks.json, and a new session reads it back.
    let file = dir.join("looks.json");
    let _ = std::fs::remove_file(&file);
    let mut a = app_with(&MESSY);
    a.cowriter.looks_file = Some(file.clone());
    a.run("ui.assistant.run", json!({"verb": "format", "look": "letter"})).unwrap();
    if let Some(v) = a.cowriter.format.as_mut() {
        v.naming = Some("  Martijn’s letters\u{7}".into());
    }
    save_look(&mut a);
    let back = read_looks(Some(&file));
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].name, "Martijn’s letters");
    assert_eq!(back[0].styles, Look::preset("letter").unwrap().styles);
    assert_eq!(state(&mut a)["look"], "Martijn’s letters");
    let _ = std::fs::remove_dir_all(&dir);
}
