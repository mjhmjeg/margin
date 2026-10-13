//! Margin's authorship store (`customXml/itemN.xml`, `urn:margin:authorship`): written only while
//! the document has accepted assistant text, read back and repaired on load, and never a reason
//! for a file to fail. Fixtures are hand-written from the OOXML package contract.
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use wordcraft_doc::authorship::{BOOKMARK_PREFIX, MAX_SPANS, NAMESPACE, Span, scan, span_text};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, Paragraph, Path, StoryRef, para_block};

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const RT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn rels(rows: &[(&str, &str, &str)]) -> String {
    let mut out = format!(r#"<Relationships xmlns="{REL_NS}">"#);
    for (id, kind, target) in rows {
        out.push_str(&format!(r#"<Relationship Id="{id}" Type="{RT}{kind}" Target="{target}"/>"#));
    }
    out + "</Relationships>"
}

/// A package with `body` and, when given, one custom XML part related from the document.
fn fixture(body: &str, custom: Option<&str>) -> BTreeMap<String, Vec<u8>> {
    let doc = format!(r#"<w:document xmlns:w="{W_NS}"><w:body>{body}</w:body></w:document>"#);
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let mut parts: Vec<(String, String)> = vec![
        ("[Content_Types].xml".into(), types.into()),
        ("_rels/.rels".into(), rels(&[("office", "officeDocument", "word/document.xml")])),
        ("word/document.xml".into(), doc),
    ];
    match custom {
        Some(xml) => {
            parts.push(("word/_rels/document.xml.rels".into(), rels(&[("data", "customXml", "../customXml/item1.xml")])));
            parts.push(("customXml/item1.xml".into(), xml.into()));
        }
        None => parts.push(("word/_rels/document.xml.rels".into(), rels(&[]))),
    }
    parts.into_iter().map(|(p, s)| (p, s.into_bytes())).collect()
}

fn zip(parts: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(bytes).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        assert!(parts.insert(name, bytes).is_none(), "duplicate ZIP entry");
    }
    parts
}

fn text(parts: &BTreeMap<String, Vec<u8>>, name: &str) -> String {
    String::from_utf8(parts.get(name).unwrap_or_else(|| panic!("no part {name}")).clone()).unwrap()
}

/// Paragraphs `paras`, with `para[a..b]` of the first wrapped in a mark and recorded.
fn marked(paras: &[&str], a: usize, b: usize, name: &str) -> Document {
    let mut d = Document::new();
    d.body = paras.iter().map(|t| para_block(Paragraph::with_text(t, CharProps::default()))).collect();
    let p = d.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
    let accepted = p.text[a..b].to_string();
    p.insert_object(b, InlineObject::BookmarkEnd { name: name.into() }, &CharProps::default()).unwrap();
    p.insert_object(a, InlineObject::BookmarkStart { name: name.into() }, &CharProps::default()).unwrap();
    d.authorship.spans.push(Span::new(name, "Claude", "claude-opus-5", "2026-10-13T09:00:00Z", &accepted));
    d
}

fn mark_name(i: u32) -> String {
    format!("{BOOKMARK_PREFIX}{i:08x}")
}

#[test]
fn spans_round_trip_and_a_second_save_is_byte_stable() {
    let d = marked(&["Mist lifts off the river by eight, and the bakery opens.", "A person wrote this."], 0, 34, &mark_name(1));
    let bytes = wordcraft_docx::write(&d).unwrap();
    let parts = unzip(&bytes);
    let item = text(&parts, "customXml/item1.xml");
    assert!(item.contains(NAMESPACE) && item.contains(&mark_name(1)) && item.contains("mg:author=\"Claude\""), "{item}");
    assert!(item.contains("<mg:text>Mist lifts off the river by eight,</mg:text>"), "the accepted text is stored: {item}");
    assert!(text(&parts, "customXml/itemProps1.xml").contains(NAMESPACE));
    assert!(text(&parts, "customXml/_rels/item1.xml.rels").contains("itemProps1.xml"));
    assert!(text(&parts, "word/_rels/document.xml.rels").contains("../customXml/item1.xml"));
    assert!(text(&parts, "[Content_Types].xml").contains("/customXml/itemProps1.xml"));
    assert!(text(&parts, "word/document.xml").contains(&format!("w:name=\"{}\"", mark_name(1))));
    let back = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(back.authorship, d.authorship);
    let sc = scan(&back);
    assert_eq!(sc.ranges.len(), 1);
    assert_eq!(span_text(&back, &sc.ranges[0]), "Mist lifts off the river by eight,");
    let again = wordcraft_docx::write(&back).unwrap();
    assert_eq!(again, bytes, "a second save writes the same bytes");
    assert_eq!(wordcraft_docx::read(&again).unwrap().authorship, d.authorship);
}

#[test]
fn edits_around_a_span_keep_it_and_the_stored_text_tells_what_changed() {
    let d = marked(&["Mist lifts off the river by eight."], 0, 34, &mark_name(7));
    let mut back = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    // An edit before the span (shifts offsets) and one inside it.
    let p = back.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
    p.insert_text(0, "Edited. ", &CharProps::default()).unwrap();
    let at = p.text.find("river").unwrap();
    p.insert_text(at, "wide ", &CharProps::default()).unwrap();
    let back2 = wordcraft_docx::read(&wordcraft_docx::write(&back).unwrap()).unwrap();
    let sc = scan(&back2);
    assert_eq!(span_text(&back2, &sc.ranges[0]), "Mist lifts off the wide river by eight.");
    let a = wordcraft_doc::authorship::attribution(&back2, &|_| false);
    assert_eq!(a.ai_chars, 34, "the five typed characters are the person's");
    assert!(a.spans[0].edited);
}

#[test]
fn a_document_without_spans_writes_no_part_and_a_plain_file_is_untouched() {
    let d = Document::from_text("Just text.");
    let parts = unzip(&wordcraft_docx::write(&d).unwrap());
    assert!(parts.keys().all(|k| !k.starts_with("customXml/")), "{:?}", parts.keys());
    assert!(!text(&parts, "word/_rels/document.xml.rels").contains("customXml"));
    // An ordinary file with an ordinary bookmark: read, saved, read again — nothing of ours appears.
    let body = r#"<w:p><w:bookmarkStart w:id="0" w:name="Intro"/><w:r><w:t>Plain words.</w:t></w:r><w:bookmarkEnd w:id="0"/></w:p>"#;
    let back = wordcraft_docx::read(&zip(&fixture(body, None))).unwrap();
    assert!(back.authorship.is_empty());
    let bytes = wordcraft_docx::write(&back).unwrap();
    let parts = unzip(&bytes);
    assert!(parts.keys().all(|k| !k.starts_with("customXml/")));
    assert!(!String::from_utf8_lossy(&bytes).contains("margin"));
    assert!(text(&parts, "word/document.xml").contains(r#"w:name="Intro""#));
    assert_eq!(wordcraft_docx::write(&wordcraft_docx::read(&bytes).unwrap()).unwrap(), bytes);
}

#[test]
fn ours_sits_beside_a_bibliography_and_a_foreign_store() {
    let mut d = marked(&["Alpha beta gamma."], 6, 10, &mark_name(2));
    d.sources.push(wordcraft_doc::Source { tag: "Tag1".into(), title: "A title".into(), ..Default::default() });
    let parts = unzip(&wordcraft_docx::write(&d).unwrap());
    assert!(text(&parts, "customXml/item1.xml").contains("b:Sources"), "the bibliography keeps item1");
    assert!(text(&parts, "customXml/item2.xml").contains(NAMESPACE));
    let back = wordcraft_docx::read(&zip(&parts)).unwrap();
    assert_eq!(back.authorship.spans.len(), 1);
    assert_eq!(back.sources.len(), 1);
    // A foreign store in a file we open stays, and ours is added next to it.
    let foreign = r#"<?xml version="1.0"?><c:customer xmlns:c="urn:original:customer"><c:name>Atlas</c:name></c:customer>"#;
    let body = format!(
        r#"<w:p><w:bookmarkStart w:id="0" w:name="{n}"/><w:r><w:t>AI text</w:t></w:r><w:bookmarkEnd w:id="0"/><w:r><w:t> and mine.</w:t></w:r></w:p>"#,
        n = mark_name(3)
    );
    let mut back = wordcraft_docx::read(&zip(&fixture(&body, Some(foreign)))).unwrap();
    // Without a part the mark is stale and goes (the text stays).
    assert!(scan(&back).ranges.is_empty() && back.authorship.is_empty());
    assert_eq!(back.plain_text(StoryRef::Body), "AI text and mine.");
    let p = back.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
    p.insert_object(7, InlineObject::BookmarkEnd { name: mark_name(4) }, &CharProps::default()).unwrap();
    p.insert_object(0, InlineObject::BookmarkStart { name: mark_name(4) }, &CharProps::default()).unwrap();
    back.authorship.spans.push(Span::new(&mark_name(4), "Claude", "", "2026-10-13T10:00:00Z", "AI text"));
    let parts = unzip(&wordcraft_docx::write(&back).unwrap());
    let ours = parts
        .iter()
        .filter(|(k, v)| k.starts_with("customXml/item") && !k.contains("Props") && String::from_utf8_lossy(v).contains(NAMESPACE))
        .count();
    let theirs = parts.iter().filter(|(_, v)| String::from_utf8_lossy(v).contains("urn:original:customer")).count();
    assert_eq!((ours, theirs), (1, 1), "{:?}", parts.keys());
    let back2 = wordcraft_docx::read(&zip(&parts)).unwrap();
    assert_eq!(back2.authorship.spans.len(), 1);
    assert_eq!(span_text(&back2, &scan(&back2).ranges[0]), "AI text");
    // And the foreign store is carried once more, ours rewritten, nothing duplicated.
    let parts3 = unzip(&wordcraft_docx::write(&back2).unwrap());
    let ours3 =
        parts3.iter().filter(|(_, v)| String::from_utf8_lossy(v).contains(NAMESPACE) && !String::from_utf8_lossy(v).contains("ds:schemaRef")).count();
    let theirs3 = parts3.iter().filter(|(_, v)| String::from_utf8_lossy(v).contains("urn:original:customer")).count();
    assert_eq!((ours3, theirs3), (1, 1), "{:?}", parts3.keys());
}

#[test]
fn load_repair_drops_what_does_not_agree() {
    // Entry with a bookmark: kept. Entry whose bookmark is missing: dropped. Collapsed bookmark:
    // dropped. Orphan end: removed. Bookmark with no entry: removed. Two entries, one name: one.
    let body = format!(
        concat!(
            r#"<w:p><w:bookmarkStart w:id="0" w:name="{a}"/><w:r><w:t>kept</w:t></w:r><w:bookmarkEnd w:id="0"/>"#,
            r#"<w:bookmarkStart w:id="1" w:name="{c}"/><w:bookmarkEnd w:id="1"/><w:r><w:t> text</w:t></w:r>"#,
            r#"<w:bookmarkEnd w:id="9"/>"#,
            r#"<w:bookmarkStart w:id="2" w:name="{s}"/><w:r><w:t> stale</w:t></w:r><w:bookmarkEnd w:id="2"/></w:p>"#
        ),
        a = mark_name(0xa),
        c = mark_name(0xc),
        s = mark_name(0x5)
    );
    let part = format!(
        concat!(
            r#"<mg:authorship xmlns:mg="{ns}" mg:version="1">"#,
            r#"<mg:span mg:ref="{a}" mg:author="Claude" mg:model="m" mg:acceptedAt="t" mg:len="4" mg:hash="x"><mg:text>kept</mg:text></mg:span>"#,
            r#"<mg:span mg:ref="{a}" mg:author="Claude"><mg:text>kept</mg:text></mg:span>"#,
            r#"<mg:span mg:ref="{m}" mg:author="Claude"><mg:text>missing</mg:text></mg:span>"#,
            r#"<mg:span mg:ref="{c}" mg:author="Claude"><mg:text>collapsed</mg:text></mg:span>"#,
            r#"</mg:authorship>"#
        ),
        ns = NAMESPACE,
        a = mark_name(0xa),
        m = mark_name(0xb),
        c = mark_name(0xc)
    );
    let back = wordcraft_docx::read(&zip(&fixture(&body, Some(&part)))).unwrap();
    assert_eq!(back.authorship.spans.iter().map(|s| s.name.clone()).collect::<Vec<_>>(), [mark_name(0xa)]);
    let sc = scan(&back);
    assert_eq!(sc.ranges.len(), 1);
    assert!(sc.stray.is_empty());
    assert_eq!(span_text(&back, &sc.ranges[0]), "kept");
    assert_eq!(back.plain_text(StoryRef::Body), "kept text stale");
    // A span's hash is recomputed from its text, so a wrong one in the file does no harm.
    assert_eq!(back.authorship.spans[0].hash, wordcraft_doc::authorship::hash("kept"));
    let parts = unzip(&wordcraft_docx::write(&back).unwrap());
    let item = text(&parts, "customXml/item1.xml");
    assert_eq!(item.matches("<mg:span").count(), 1, "{item}");
}

#[test]
fn hostile_parts_never_panic_and_leave_the_text_alone() {
    let n = mark_name(0x11);
    let body =
        format!(r#"<w:p><w:bookmarkStart w:id="0" w:name="{n}"/><w:r><w:t>AI</w:t></w:r><w:bookmarkEnd w:id="0"/><w:r><w:t> ok</w:t></w:r></w:p>"#);
    let many: String = (0..MAX_SPANS + 5).map(|i| format!(r#"<mg:span mg:ref="{}"><mg:text>t</mg:text></mg:span>"#, mark_name(i as u32))).collect();
    let long = "y".repeat(wordcraft_doc::authorship::MAX_STORED_TEXT_CHARS + 1);
    let cases: Vec<(&str, String)> = vec![
        ("malformed", format!(r#"<mg:authorship xmlns:mg="{NAMESPACE}"><mg:span mg:ref="{n}">"#)),
        ("not xml", "\u{0}\u{1}garbage<<<".into()),
        ("empty", String::new()),
        (
            "wrong namespace",
            format!(r#"<mg:authorship xmlns:mg="urn:someone:else"><mg:span mg:ref="{n}"><mg:text>AI</mg:text></mg:span></mg:authorship>"#),
        ),
        ("other root", format!(r#"<mg:spans xmlns:mg="{NAMESPACE}"><mg:span mg:ref="{n}"><mg:text>AI</mg:text></mg:span></mg:spans>"#)),
        ("too many", format!(r#"<mg:authorship xmlns:mg="{NAMESPACE}">{many}</mg:authorship>"#)),
        (
            "bad ref",
            format!(
                r#"<mg:authorship xmlns:mg="{NAMESPACE}"><mg:span mg:ref="Intro"><mg:text>AI</mg:text></mg:span><mg:span><mg:text>AI</mg:text></mg:span></mg:authorship>"#
            ),
        ),
        ("no text no hash", format!(r#"<mg:authorship xmlns:mg="{NAMESPACE}"><mg:span mg:ref="{n}" mg:hash="zz"/></mg:authorship>"#)),
        (
            "hash only",
            format!(
                r#"<mg:authorship xmlns:mg="{NAMESPACE}"><mg:span mg:ref="{n}" mg:hash="{}" mg:len="2"/></mg:authorship>"#,
                wordcraft_doc::authorship::hash("AI")
            ),
        ),
        (
            "long text",
            format!(
                r#"<mg:authorship xmlns:mg="{NAMESPACE}"><mg:span mg:ref="{n}" mg:hash="0123456789abcdef" mg:len="3"><mg:text>{long}</mg:text></mg:span></mg:authorship>"#
            ),
        ),
        ("deep", format!(r#"<mg:authorship xmlns:mg="{NAMESPACE}">{}{}</mg:authorship>"#, "<mg:span>".repeat(2000), "</mg:span>".repeat(2000))),
        (
            "huge attrs",
            format!(
                r#"<mg:authorship xmlns:mg="{NAMESPACE}"><mg:span mg:ref="{n}{}" mg:author="{}"><mg:text>AI</mg:text></mg:span></mg:authorship>"#,
                "z".repeat(5000),
                "a".repeat(100_000)
            ),
        ),
    ];
    for (what, part) in cases {
        let doc = wordcraft_docx::read(&zip(&fixture(&body, Some(&part)))).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(doc.plain_text(StoryRef::Body), "AI ok", "{what}");
        let bytes = wordcraft_docx::write(&doc).unwrap_or_else(|e| panic!("{what}: {e}"));
        let back = wordcraft_docx::read(&bytes).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(back.authorship, doc.authorship, "{what}");
        match what {
            "hash only" => {
                assert_eq!(doc.authorship.spans.len(), 1, "{what}");
                assert!(doc.authorship.spans[0].text.is_none());
                assert_eq!(scan(&doc).ranges.len(), 1);
                let a = wordcraft_doc::authorship::attribution(&doc, &|_| false);
                assert_eq!(a.ai_chars, 2, "a hash-only span counts as the AI's as a whole");
            }
            "long text" => {
                assert_eq!(doc.authorship.spans.len(), 1, "{what}");
                assert!(doc.authorship.spans[0].text.is_none(), "over the cap: only the hash is kept");
                assert!(wordcraft_doc::authorship::attribution(&doc, &|_| false).spans[0].edited);
            }
            "too many" => {
                assert_eq!(doc.authorship.spans.len(), 1, "{what}: only the span with a bookmark stays");
                assert!(scan(&doc).ranges.len() == 1);
            }
            "huge attrs" => {
                assert!(doc.authorship.is_empty() && scan(&doc).ranges.is_empty(), "{what}: the ref is not the bookmark's name");
            }
            "wrong namespace" | "other root" => {
                // Not ours: carried through as a foreign store; the mark is stale and goes.
                assert!(doc.authorship.is_empty() && scan(&doc).ranges.is_empty(), "{what}");
                let parts = unzip(&bytes);
                assert!(parts.values().any(|v| String::from_utf8_lossy(v).contains("mg:span")), "{what}: foreign store kept");
            }
            _ => assert!(doc.authorship.is_empty() && scan(&doc).ranges.is_empty(), "{what}: {:?}", doc.authorship),
        }
    }
}
