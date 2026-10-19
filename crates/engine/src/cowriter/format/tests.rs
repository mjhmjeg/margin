//! Format for me's rules on messy synthetic documents (a pasted email, a plain-text list,
//! aligned columns, mixed fonts), and the identity check around them (rule 5).

use serde_json::json;
use wordcraft_doc::{Block, Document, StoryRef};

use super::*;
use crate::assistant_edits::{ScopeKind, scope};
use crate::cowriter::{Call, Landed, Verb, apply, plain_words, run};

const NAME: &str = "AI · Claude (co-writer)";

fn session(paras: &[&str]) -> Session {
    let mut s = Session::new(Document::new());
    s.author = "Martijn".into();
    s.prefs.autocorrect.enabled = false;
    for (i, p) in paras.iter().enumerate() {
        if i > 0 {
            s.run("text.newParagraph", &json!({})).unwrap();
        }
        s.run("text.insert", &json!({"text": p, "raw": true})).unwrap();
    }
    s
}

/// Format paragraph `i` (all of it) with `cmd`.
fn fmt(s: &mut Session, i: usize, cmd: &str, v: serde_json::Value) {
    let len = para(s, i).text.len();
    s.run("select.range", &json!({"anchor": pos(i, 0), "focus": pos(i, len)})).unwrap();
    s.run(cmd, &v).unwrap();
    s.sel = crate::Selection::caret(pos(0, 0));
}

fn para(s: &Session, i: usize) -> &wordcraft_doc::Paragraph {
    match s.doc.body.get(i).map(|b| &**b) {
        Some(Block::Para(p)) => p,
        _ => panic!("no paragraph {i}"),
    }
}

/// The paragraph whose text as it reads is `t`.
fn find<'a>(s: &'a Session, t: &str) -> &'a wordcraft_doc::Paragraph {
    s.doc
        .body
        .iter()
        .find_map(|b| match &**b {
            Block::Para(p) if p.final_text() == t => Some(p),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no paragraph “{t}”"))
}

fn style(s: &Session, t: &str) -> String {
    find(s, t).props.style.clone().unwrap_or_default()
}

fn words(s: &Session) -> Vec<String> {
    plain_words(&final_text(s))
}

fn final_text(s: &Session) -> String {
    crate::cowriter::final_body(&s.doc.body, 0)
}

fn format_run(s: &mut Session, kind: ScopeKind, options: serde_json::Value) -> (Ran, Landed) {
    let sc = scope(s, kind).unwrap();
    let ran = run(Verb::FormatForMe, &Call { session: s, scope: &sc, options: &options, name: NAME }, None).unwrap();
    let landed = apply(s, NAME, Verb::FormatForMe, &sc, ran.outputs.clone());
    (ran, landed)
}

/// A pasted email: bold and ALL-CAPS headings, a typed list, aligned columns, empty lines for
/// spacing, straight quotes, three dots and spaced hyphens.
fn email() -> Session {
    let mut s = session(&[
        "Studio rules",
        "Spring update",
        "",
        "THE KILN",
        "Book the kiln a week ahead. Don't leave \"wet\" pots on the shelf...",
        "",
        "Bring your own",
        "- Clay",
        "- Glaze",
        "* Towels",
        "",
        "Costs",
        "Members pay 30 a month - guests  pay 10.",
        "Name   Role     Day",
        "Anna   Firing   Mon",
        "Ben    Glaze    Tue",
        "1) Sweep the floor",
        "2) Lock up",
        "Thanks, Martijn",
    ]);
    fmt(&mut s, 6, "format.bold", json!({"value": true}));
    fmt(&mut s, 11, "format.bold", json!({"value": true}));
    s
}

#[test]
fn a_pasted_email_gets_headings_lists_a_table_and_typography_and_keeps_its_words() {
    let mut s = email();
    let before = words(&s);
    let undo = s.undo_depth();
    let (ran, landed) = format_run(&mut s, ScopeKind::Document, json!({"look": "report"}));
    assert_eq!(landed.words, Some((before.len(), 0)), "{:?} {:?}", landed.lines, ran.lines);
    assert!(landed.lines.is_empty(), "{:?}", landed.lines);
    assert_eq!(words(&s), before, "rule 5");
    assert_eq!(s.undo_depth(), undo + 1, "one undo step");
    // Title, subtitle, headings by emphasis: ALL-CAPS above bold.
    assert_eq!(style(&s, "Studio rules"), "Title");
    assert_eq!(style(&s, "Spring update"), "Subtitle");
    assert_eq!(style(&s, "THE KILN"), "Heading1");
    assert_eq!(style(&s, "Bring your own"), "Heading2");
    assert_eq!(style(&s, "Costs"), "Heading2");
    // Real lists, the typed markers gone.
    for t in ["Clay", "Glaze", "Towels", "Sweep the floor", "Lock up"] {
        assert!(find(&s, t).props.numbering.is_some_and(|n| n.num != 0), "{t}");
    }
    assert_eq!(style(&s, "Clay"), s.doc.styles.find("List Bullet").unwrap().id);
    assert_eq!(style(&s, "Lock up"), s.doc.styles.find("List Number").unwrap().id);
    // The aligned columns are a 3 × 3 table in the look's table style, with a header row.
    let t = s
        .doc
        .body
        .iter()
        .find_map(|b| match &**b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("a table");
    assert_eq!((t.rows.len(), t.rows[0].cells.len()), (3, 3));
    assert_eq!(t.props.style.as_deref(), s.doc.styles.find(TABLE_STYLE).map(|x| x.id.as_str()));
    assert!(t.props.look.header_row);
    // Typography, as it reads.
    let text = final_text(&s);
    assert!(text.contains("Don’t leave “wet” pots on the shelf…"), "{text}");
    assert!(text.contains("30 a month – guests pay 10."), "{text}");
    // Header with the title, page numbers in the footer.
    let sect = &s.doc.last_section;
    let h = sect.headers.default.expect("a header");
    assert_eq!(s.doc.plain_text(StoryRef::Part(h)), "Studio rules");
    assert!(sect.footers.default.is_some());
    // Everything in the body is the co-writer's, tracked.
    let changes = s.run("review.changes", &json!({})).unwrap();
    let list = changes.as_array().unwrap();
    assert!(!list.is_empty());
    assert!(list.iter().all(|c| c["author"] == NAME), "{changes}");
    assert_eq!(s.author, "Martijn");
    assert!(!s.doc.settings.track_changes, "the person's tracking is back");
    // Accepting everything leaves no spacing paragraphs and the same words.
    s.run("review.acceptAll", &json!({})).unwrap();
    assert_eq!(words(&s), before);
    let empties = s.doc.body.iter().filter(|b| matches!(&***b, Block::Para(p) if p.text.is_empty())).count();
    assert_eq!(empties, 0, "{:?}", s.doc.plain_text(StoryRef::Body));
    // The joins kept the paragraphs after them as they were.
    assert_eq!(style(&s, "THE KILN"), "Heading1");
    assert_eq!(style(&s, "Costs"), "Heading2");
}

#[test]
fn the_whole_run_is_taken_back_with_one_undo() {
    let mut s = email();
    let before = s.doc.plain_text(StoryRef::Body);
    format_run(&mut s, ScopeKind::Document, json!({"look": "clean"}));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc.plain_text(StoryRef::Body), before);
    assert!(s.run("review.changes", &json!({})).unwrap().as_array().unwrap().is_empty());
}

#[test]
fn a_plain_text_list_becomes_a_numbered_and_a_lettered_list() {
    let mut s = session(&["Steps", "1. Wedge the clay", "2. Centre it", "3. Open it up", "a) slowly", "b) wet hands", "Done."]);
    let before = words(&s);
    let (_, landed) = format_run(&mut s, ScopeKind::Document, json!({"look": "clean"}));
    assert_eq!(landed.words, Some((before.len(), 0)));
    let num = |t: &str| find(&s, t).props.numbering.map(|n| n.num).unwrap_or(0);
    assert!(num("Wedge the clay") != 0 && num("Wedge the clay") == num("Open it up"));
    assert!(num("slowly") != 0 && num("slowly") != num("Wedge the clay"), "a) is its own list");
}

#[test]
fn tab_separated_columns_become_a_table() {
    let mut s = session(&["Prices", "Item\tPrice", "Clay\t30", "Glaze\t12", "That is all."]);
    let before = words(&s);
    let (_, landed) = format_run(&mut s, ScopeKind::Document, json!({"look": "clean"}));
    assert_eq!(landed.words, Some((before.len(), 0)));
    let t = s
        .doc
        .body
        .iter()
        .find_map(|b| match &**b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("a table");
    assert_eq!((t.rows.len(), t.rows[0].cells.len()), (3, 2));
}

#[test]
fn mixed_fonts_take_the_looks_fonts_and_code_keeps_monospace() {
    let mut s = session(&["Notes", "Some text in a pasted font.", "let x = 1;", "More text."]);
    fmt(&mut s, 1, "format.font", json!({"name": "Comic Neue"}));
    fmt(&mut s, 2, "format.font", json!({"name": "Courier New"}));
    fmt(&mut s, 3, "format.color", json!({"color": "C00000"}));
    format_run(&mut s, ScopeKind::Document, json!({"look": "classic"}));
    let font = |i: usize| para(&s, i).runs.iter().find_map(|r| r.props.font.clone());
    assert_eq!(font(1), None, "the pasted font goes");
    assert_eq!(font(2).as_deref(), Some("Courier New"), "code keeps monospace");
    let colour = para(&s, 3).runs.iter().find_map(|r| r.props.color);
    assert!(colour.is_some(), "Keep my colours is on by default");
    // With Keep my colours off, the colour goes too.
    let mut s2 = session(&["Notes", "Red text."]);
    fmt(&mut s2, 1, "format.color", json!({"color": "C00000"}));
    format_run(&mut s2, ScopeKind::Document, json!({"look": "classic", "keepColours": false}));
    assert!(para(&s2, 1).runs.iter().all(|r| r.props.color.is_none()));
}

#[test]
fn a_selection_formats_only_what_is_selected() {
    let mut s = session(&["Intro", "Keep \"this\" as it is.", "- one", "- two", "Fix \"this\" one."]);
    let len = para(&s, 4).text.len();
    s.run("select.range", &json!({"anchor": pos(2, 0), "focus": pos(4, len)})).unwrap();
    format_run(&mut s, ScopeKind::Selection, json!({"look": "clean"}));
    assert_eq!(para(&s, 1).final_text(), "Keep \"this\" as it is.");
    assert_eq!(para(&s, 4).final_text(), "Fix “this” one.");
    assert!(para(&s, 2).props.numbering.is_some());
    assert_ne!(style(&s, "Intro"), "Title", "no title outside the whole document");
    assert!(s.doc.last_section.headers.default.is_none());
}

#[test]
fn a_rule_that_would_change_words_changes_nothing() {
    // A deliberately bad rule: converting at commas drops the commas from the words.
    let mut s = session(&["Clay, glaze, towels", "Anna, Ben, Carla"]);
    let before = s.doc.plain_text(StoryRef::Body);
    let undo = s.undo_depth();
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let bad = vec![Output::FormatChange {
        command: "table.fromText".into(),
        params: json!({"separator": "comma", "first": true, "targets": [[pos(0, 0), pos(1, 16)]]}),
    }];
    let landed = apply(&mut s, NAME, Verb::FormatForMe, &sc, bad);
    let (kept, changed) = landed.words.unwrap();
    assert!(changed > 0 && kept < 6, "{:?}", landed.words);
    assert!(landed.lines.iter().any(|l| l.contains("stopped")), "{:?}", landed.lines);
    assert_eq!(s.doc.plain_text(StoryRef::Body), before);
    assert_eq!(s.undo_depth(), undo);
    // And format.typography itself refuses a swap that changes a word.
    let e = s.run("format.typography", &json!({"edits": [{"from": pos(0, 0), "to": pos(0, 4), "with": "Mud"}]}));
    assert!(e.is_err());
    assert_eq!(s.doc.plain_text(StoryRef::Body), before);
}

#[test]
fn switching_the_look_is_one_undo_step_and_writes_the_style_table() {
    let mut s = email();
    format_run(&mut s, ScopeKind::Document, json!({"look": "clean"}));
    let h1 = |s: &Session| s.doc.styles.find("Heading 1").and_then(|x| x.chr.font.clone());
    assert_eq!(h1(&s).as_deref(), Some("Source Sans 3"));
    let undo = s.undo_depth();
    let text = s.doc.plain_text(StoryRef::Body);
    let (ran, landed) = format_run(&mut s, ScopeKind::Document, json!({"look": "classic", "lookOnly": true}));
    assert_eq!(ran.report, "Look: Classic");
    assert_eq!(landed.words.map(|w| w.1), Some(0));
    assert_eq!(h1(&s).as_deref(), Some("Source Serif 4"));
    assert_eq!(s.doc.plain_text(StoryRef::Body), text, "only the styles");
    assert_eq!(s.undo_depth(), undo + 1);
    // The saved form of what the document has now is the preset itself.
    let saved = Look::from_doc(&s.doc, "classic");
    assert_eq!(saved, Look::preset("classic").unwrap());
    // And it round-trips through the docx style table.
    let bytes = crate::io::save_bytes("x.docx", &s.doc).unwrap();
    let back = crate::io::open_bytes("x.docx", &bytes).unwrap();
    assert_eq!(Look::from_doc(&back, "classic"), saved);
}

#[test]
fn a_saved_look_applies_like_a_preset() {
    let mut s = session(&["Studio rules", "Book the kiln."]);
    let mut mine = Look::preset("letter").unwrap();
    mine.name = "Martijn’s letters".into();
    mine.styles.iter_mut().for_each(|x| x.color = "7A1F1F".into());
    let (ran, _) = format_run(&mut s, ScopeKind::Document, json!({"look": mine}));
    assert_eq!(ran.report, "Look: Martijn’s letters");
    let c = s.doc.styles.find("Title").and_then(|x| x.chr.color);
    assert_eq!(c, Some(TextColor::Rgb(wordcraft_doc::props::Rgb(0x7A, 0x1F, 0x1F))));
    // Garbage instead of a look: Clean, no panic.
    let mut s = session(&["Studio rules", "Book the kiln."]);
    let (ran, _) = format_run(&mut s, ScopeKind::Document, json!({"look": {"name": 5, "styles": "x"}}));
    assert_eq!(ran.report, "Look: Clean");
}

#[test]
fn the_model_only_guesses_the_look() {
    let s = email();
    let sc = scope(&s, ScopeKind::Document).unwrap();
    let call = Call { session: &s, scope: &sc, options: &json!({}), name: NAME };
    let p = crate::cowriter::prompt(Verb::FormatForMe, &call).unwrap().expect("a prompt");
    assert!(p.max_tokens <= 100 && p.user.contains("BEGIN DOCUMENT"));
    let given = json!({"look": "letter"});
    assert!(crate::cowriter::prompt(Verb::FormatForMe, &Call { options: &given, ..call }).unwrap().is_none(), "no model when the look is given");
    let read = |t: &str| run(Verb::FormatForMe, &call, Some(Reply { text: t, stop_reason: None })).unwrap().report;
    assert_eq!(read(r#"{"look": "report", "reads_like": "Reads like a handbook"}"#), "Reads like a handbook · Look: Report");
    assert_eq!(read("```json\n{\"look\":\"Letter\"}\n```"), "Look: Letter");
    assert_eq!(read("I think it is a novel"), "Look: Clean");
    assert_eq!(read(r#"{"look": "rm -rf", "reads_like": "x\ny"}"#), "xy · Look: Clean");
    assert_eq!(run(Verb::FormatForMe, &call, None).unwrap().report, "Look: Clean", "without a model");
}

#[test]
fn word_changes_become_check_lines() {
    let mut s = session(&["Notes", "Book the the kiln early.", "Costs.", "Members pay monthly."]);
    fmt(&mut s, 2, "format.bold", json!({"value": true}));
    let (ran, _) = format_run(&mut s, ScopeKind::Document, json!({"look": "clean"}));
    assert!(ran.lines.iter().any(|l| l.contains("“the the”")), "{:?}", ran.lines);
    assert!(ran.lines.iter().any(|l| l.contains("Costs.")), "{:?}", ran.lines);
    assert_eq!(para(&s, 1).final_text(), "Book the the kiln early.", "never fixed");
}

#[test]
fn number_the_headings_links_the_heading_styles_to_a_list() {
    let mut s = email();
    format_run(&mut s, ScopeKind::Document, json!({"look": "report", "numberHeadings": true}));
    assert!(s.doc.styles.find("Heading 1").and_then(|x| x.para.numbering).is_some());
}

#[test]
fn markers_columns_and_quotes() {
    assert_eq!(marker("- Clay"), Some((2, "bullet")));
    assert_eq!(marker("  •  Clay"), Some((7, "bullet")));
    assert_eq!(marker("12) Sweep"), Some((4, "numberedParen")));
    assert_eq!(marker("3. Open"), Some((3, "numbered")));
    assert_eq!(marker("b) wet"), Some((3, "lowerLetter")));
    for no in ["-5 degrees", "2026. A year", "B) upper", "- ", "1234. x", "e.g. this", "–"] {
        assert_eq!(marker(no), None, "{no}");
    }
    assert_eq!(columns("Name   Role  Day"), Some((false, 3)));
    assert_eq!(columns("a\tb\t\tc"), Some((true, 4)));
    assert_eq!(columns("Just one  gap"), Some((false, 2)));
    assert_eq!(columns("No gaps here"), None);
    let ed = |t: &str, lang: &str| {
        let mut out = t.to_string();
        for (a, b, w) in typography(t, 0, quotes(lang)).into_iter().rev() {
            out.replace_range(a..b, &w);
        }
        out
    };
    assert_eq!(ed("She said \"yes\" and 'no'. It's Martijn's...", "en-US"), "She said “yes” and ‘no’. It’s Martijn’s…");
    assert_eq!(ed("Er sagte \"ja\" - oder 'nein'", "de-DE"), "Er sagte „ja“ – oder ‚nein‘");
    assert_eq!(ed("Sie sagte \"ja\"", "de-CH"), "Sie sagte «ja»");
    assert_eq!(ed("Wait....  Really?  ", "en"), "Wait.... Really?  ");
    assert_eq!(ed("(\"quoted\")", "en"), "(“quoted”)");
    assert_eq!(ed("well-known 3-5", "en"), "well-known 3-5");
}

/// Hostile documents: huge, empty, objects and nesting; nothing panics and words stay.
#[test]
fn odd_documents_never_panic() {
    for paras in [vec![""], vec!["", "", ""], vec!["\"", "'", "..."], vec!["-", "1)", "a)"], vec!["x\t", "\ty", "  "]] {
        let mut s = session(&paras);
        let before = words(&s);
        if scope(&s, ScopeKind::Document).is_err() {
            continue;
        }
        let (_, landed) = format_run(&mut s, ScopeKind::Document, json!({"look": "clean"}));
        assert_eq!(landed.words.map(|w| w.1), Some(0), "{paras:?}: {:?}", landed.lines);
        assert_eq!(words(&s), before, "{paras:?}");
    }
}
