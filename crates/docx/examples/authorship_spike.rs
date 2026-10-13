//! A5 spike harness: how "this text was written by the AI, accepted at T" can be stored in a
//! `.docx` so that it survives Word and upstream WordCraft (docs/authorship-storage.md).
//!
//! ```text
//! authorship_spike make <outdir>       write one synthetic test document per candidate
//! authorship_spike check <file.docx>   report the authorship marks in a package + OOXML sanity
//! ```
//!
//! The documents are built with this crate's writer; what the writer has no API for (a custom
//! XML part of our own, ignorable attributes on runs) is patched into the package afterwards,
//! from the public OOXML contract (ECMA-376 Part 1 §17.13.6 bookmarks, §17.5.2 content controls,
//! §11.3.5 custom XML data storage; Part 3 markup compatibility). The text is our own.
//!
//! `check` prints one stable line per finding so two reports (before / after a round trip)
//! can be diffed, and ends with a `summary` line for tables.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::{NsReader, XmlVersion};
use wordcraft_doc::props::CharProps;
use wordcraft_doc::styles::{Style, StyleKind};
use wordcraft_doc::{ContentControl, ControlKind, Document, InlineObject, Paragraph, para_block};

type Res<T> = Result<T, Box<dyn Error>>;

/// Our namespace for authorship metadata (custom XML part, ignorable attributes).
const MG_NS: &str = "urn:margin:authorship";
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const W15_NS: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const RT_CUSTOM_XML: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
const RT_CUSTOM_XML_PROPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
const CT_CUSTOM_XML_PROPS: &str = "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";

/// Fixed values so every run writes identical files.
const AUTHOR: &str = "Claude";
const MODEL: &str = "claude-opus-5";
const ACCEPTED_AT: &str = "2026-10-13T09:00:00Z";
const ITEM_ID: &str = "{6D1F3B2A-0C4E-4F8B-9A7D-2E5C1B3F4A60}";
/// Bookmark names starting with `_` are hidden in Word's Bookmark dialog.
const BOOKMARK_PREFIX: &str = "_MarginAI";
const STYLE_ID: &str = "MarginAI";

const CANDIDATES: &[&str] = &["a-bookmarks", "b-offsets", "c-ignorable", "d-sdt", "e-style"];

const USAGE: &str = "usage: authorship_spike make <outdir> | check <file.docx>";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match (args.first().map(String::as_str), args.get(1)) {
        (Some("make"), Some(dir)) => make(dir),
        (Some("check"), Some(file)) => check(file),
        _ => Err(USAGE.into()),
    };
    if let Err(e) = r {
        eprintln!("authorship_spike: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------- the synthetic document

/// A piece of the document: text, formatting, and the AI span (0-based) it belongs to.
struct Seg {
    text: &'static str,
    bold: bool,
    ai: Option<usize>,
}

const fn seg(text: &'static str, bold: bool, ai: Option<usize>) -> Seg {
    Seg { text, bold, ai }
}

/// Four paragraphs, three AI spans: one mid-paragraph, one whole paragraph, one over two runs
/// with different formatting.
fn content() -> Vec<Vec<Seg>> {
    vec![
        vec![
            seg("The valley wakes slowly. ", false, None),
            seg("Mist lifts off the river by eight,", false, Some(0)),
            seg(" and the bakery opens at nine.", false, None),
        ],
        vec![seg("Seewis sits above the Prättigau, a village of six hundred people and a view that pays for the climb.", false, Some(1))],
        vec![
            seg("Notes for the trip: ", false, None),
            seg("pack a map and ", false, Some(2)),
            seg("good boots", true, Some(2)),
            seg(" before Friday.", false, None),
        ],
        vec![seg("A closing line written by a person.", false, None)],
    ]
}

/// Where a span sits in the text: paragraph index, char offsets, text.
struct SpanInfo {
    idx: usize,
    para: usize,
    start: usize,
    end: usize,
    text: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Mark {
    /// Nothing in the body (candidate B).
    None,
    /// Hidden bookmarks around the spans (A, and the base for C).
    Bookmark,
    /// Inline rich-text content controls with a tag and hidden appearance (D).
    Sdt,
    /// A hidden character style on the runs (E).
    Style,
}

fn bookmark_name(idx: usize) -> String {
    format!("{BOOKMARK_PREFIX}{}", idx + 1)
}

fn sdt_tag() -> String {
    format!("margin.ai;author={AUTHOR};model={MODEL};at={ACCEPTED_AT}")
}

fn control(idx: usize) -> ContentControl {
    let mut c = ContentControl::new(ControlKind::RichText);
    c.id = Some(1001 + idx as i64);
    c.tag = sdt_tag();
    // [MS-DOCX] w15:appearance: no bounding box or title tab in Word 2013+.
    c.extra.push(r#"<w15:appearance w15:val="hidden"/>"#.to_string());
    c
}

fn build(mark: Mark) -> Res<(Document, Vec<SpanInfo>)> {
    let mut doc = Document::new();
    if mark == Mark::Style {
        doc.styles.styles.push(Style {
            id: STYLE_ID.into(),
            name: "Margin AI".into(),
            kind: StyleKind::Character,
            hidden: true,
            ..Default::default()
        });
    }
    let mut spans: Vec<SpanInfo> = Vec::new();
    let mut paras = Vec::new();
    for (pi, segs) in content().into_iter().enumerate() {
        let mut p = Paragraph::new();
        let mut chars = 0usize;
        let mut open: Option<usize> = None;
        for s in segs {
            if s.ai != open {
                if let Some(i) = open.take() {
                    close_span(&mut p, mark, i)?;
                }
                if let Some(i) = s.ai {
                    open_span(&mut p, mark, i)?;
                    spans.push(SpanInfo { idx: i, para: pi, start: chars, end: chars, text: String::new() });
                    open = Some(i);
                }
            }
            let mut props = CharProps { bold: s.bold.then_some(true), ..Default::default() };
            if mark == Mark::Style && s.ai.is_some() {
                props.style = Some(STYLE_ID.into());
            }
            let off = p.len();
            p.insert_text(off, s.text, &props)?;
            chars += s.text.chars().count();
            if let (Some(i), Some(info)) = (s.ai, spans.last_mut())
                && info.idx == i
            {
                info.end = chars;
                info.text.push_str(s.text);
            }
        }
        if let Some(i) = open {
            close_span(&mut p, mark, i)?;
        }
        paras.push(p);
    }
    doc.body = paras.into_iter().map(para_block).collect();
    Ok((doc, spans))
}

fn open_span(p: &mut Paragraph, mark: Mark, idx: usize) -> Res<()> {
    let off = p.len();
    match mark {
        Mark::Bookmark => p.insert_object(off, InlineObject::BookmarkStart { name: bookmark_name(idx) }, &CharProps::default())?,
        Mark::Sdt => p.insert_object(off, InlineObject::ControlStart { control: Box::new(control(idx)) }, &CharProps::default())?,
        Mark::None | Mark::Style => {}
    }
    Ok(())
}

fn close_span(p: &mut Paragraph, mark: Mark, idx: usize) -> Res<()> {
    let off = p.len();
    match mark {
        Mark::Bookmark => p.insert_object(off, InlineObject::BookmarkEnd { name: bookmark_name(idx) }, &CharProps::default())?,
        Mark::Sdt => p.insert_object(off, InlineObject::ControlEnd, &CharProps::default())?,
        Mark::None | Mark::Style => {}
    }
    Ok(())
}

// ---------------------------------------------------------------- make

fn make(dir: &str) -> Res<()> {
    std::fs::create_dir_all(dir)?;
    for name in CANDIDATES {
        let bytes = candidate(name)?;
        let path = format!("{dir}/{name}.docx");
        std::fs::write(&path, bytes)?;
        println!("wrote {path}");
    }
    Ok(())
}

fn candidate(name: &str) -> Res<Vec<u8>> {
    Ok(match name {
        "a-bookmarks" => {
            let (doc, spans) = build(Mark::Bookmark)?;
            let mut parts = unzip(&wordcraft_docx::write(&doc)?)?;
            add_custom_part(&mut parts, &spans_xml(&spans, "bookmark", |s| bookmark_name(s.idx)))?;
            zip(&parts)?
        }
        "b-offsets" => {
            let (doc, spans) = build(Mark::None)?;
            let mut parts = unzip(&wordcraft_docx::write(&doc)?)?;
            add_custom_part(&mut parts, &spans_xml(&spans, "offset", |_| String::new()))?;
            zip(&parts)?
        }
        "c-ignorable" => {
            let (doc, spans) = build(Mark::Bookmark)?;
            let mut parts = unzip(&wordcraft_docx::write(&doc)?)?;
            ignorable_attributes(&mut parts, &spans)?;
            zip(&parts)?
        }
        "d-sdt" => {
            let (doc, _) = build(Mark::Sdt)?;
            wordcraft_docx::write(&doc)?
        }
        "e-style" => {
            let (doc, spans) = build(Mark::Style)?;
            let mut parts = unzip(&wordcraft_docx::write(&doc)?)?;
            add_custom_part(&mut parts, &spans_xml(&spans, "style", |_| STYLE_ID.to_string()))?;
            zip(&parts)?
        }
        other => return Err(format!("unknown candidate {other}").into()),
    })
}

/// The custom XML data part: one `mg:span` per AI span. `anchor` says how a span is found in
/// the body; `offset` spans also carry paragraph index and char offsets. Every span carries the
/// length and FNV-1a hash of its accepted text so a reader can tell whether it was edited.
fn spans_xml(spans: &[SpanInfo], anchor: &str, reference: impl Fn(&SpanInfo) -> String) -> String {
    let mut x = String::new();
    let _ = write!(x, r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><mg:authorship xmlns:mg="{MG_NS}" mg:version="1">"#);
    for s in spans {
        let _ = write!(x, r#"<mg:span mg:anchor="{anchor}" mg:ref="{}""#, esc(&reference(s)));
        if anchor == "offset" {
            let _ = write!(x, r#" mg:para="{}" mg:start="{}" mg:end="{}""#, s.para, s.start, s.end);
        }
        let _ = write!(
            x,
            r#" mg:author="{AUTHOR}" mg:kind="ai" mg:acceptedAt="{ACCEPTED_AT}" mg:model="{MODEL}" mg:len="{}" mg:hash="{}"/>"#,
            s.text.chars().count(),
            fnv1a(&s.text)
        );
    }
    x.push_str("</mg:authorship>");
    x
}

/// Add `customXml/itemN.xml` (+ its properties part and relationships), relate it from the
/// main document part, and register the properties part's content type.
fn add_custom_part(parts: &mut Vec<(String, Vec<u8>)>, data: &str) -> Res<()> {
    let n = (1..100).find(|i| !parts.iter().any(|(p, _)| p.eq_ignore_ascii_case(&format!("customXml/item{i}.xml")))).ok_or("no free item")?;
    let props = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{ITEM_ID}"><ds:schemaRefs><ds:schemaRef ds:uri="{MG_NS}"/></ds:schemaRefs></ds:datastoreItem>"#
    );
    let item_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{REL_NS}"><Relationship Id="rId1" Type="{RT_CUSTOM_XML_PROPS}" Target="itemProps{n}.xml"/></Relationships>"#
    );
    parts.push((format!("customXml/item{n}.xml"), data.as_bytes().to_vec()));
    parts.push((format!("customXml/itemProps{n}.xml"), props.into_bytes()));
    parts.push((format!("customXml/_rels/item{n}.xml.rels"), item_rels.into_bytes()));
    patch(parts, "word/_rels/document.xml.rels", |s| {
        let rel = format!(r#"<Relationship Id="rIdMargin{n}" Type="{RT_CUSTOM_XML}" Target="../customXml/item{n}.xml"/></Relationships>"#);
        s.replacen("</Relationships>", &rel, 1)
    })?;
    patch(parts, "[Content_Types].xml", |s| {
        let ov = format!(r#"<Override PartName="/customXml/itemProps{n}.xml" ContentType="{CT_CUSTOM_XML_PROPS}"/></Types>"#);
        s.replacen("</Types>", &ov, 1)
    })
}

/// Candidate C: turn the bookmarks into `mg:` attributes on every run they enclose, declare the
/// namespace on the root, list it in `mc:Ignorable` and ask for preservation
/// (ECMA-376 Part 3 §10.1.2, `mc:PreserveAttributes`).
fn ignorable_attributes(parts: &mut [(String, Vec<u8>)], spans: &[SpanInfo]) -> Res<()> {
    patch(parts, "word/document.xml", |s| {
        let mut s = s.to_string();
        let marker = r#"mc:Ignorable="w14 w15 wp14""#;
        if !s.contains(marker) {
            return s;
        }
        s = s.replacen(marker, &format!(r#"xmlns:mg="{MG_NS}" mc:Ignorable="w14 w15 wp14 mg" mc:PreserveAttributes="mg:*""#), 1);
        for sp in spans {
            let name_attr = format!(r#" w:name="{}"/>"#, bookmark_name(sp.idx));
            let Some(name_at) = s.find(&name_attr) else { continue };
            let Some(start_at) = s[..name_at].rfind("<w:bookmarkStart ") else { continue };
            let start_tag = s[start_at..name_at + name_attr.len()].to_string();
            let Some(id) = start_tag.split("w:id=\"").nth(1).and_then(|r| r.split('"').next()) else { continue };
            let end_tag = format!(r#"<w:bookmarkEnd w:id="{id}"/>"#);
            let Some(end_rel) = s[start_at..].find(&end_tag) else { continue };
            let end_at = start_at + end_rel;
            let inner = s[start_at + start_tag.len()..end_at]
                .replace("<w:r>", &format!(r#"<w:r mg:author="{AUTHOR}" mg:kind="ai" mg:acceptedAt="{ACCEPTED_AT}" mg:model="{MODEL}">"#));
            s = format!("{}{}{}", &s[..start_at], inner, &s[end_at + end_tag.len()..]);
        }
        s
    })
}

fn patch(parts: &mut [(String, Vec<u8>)], name: &str, f: impl Fn(&str) -> String) -> Res<()> {
    let (_, bytes) = parts.iter_mut().find(|(p, _)| p == name).ok_or_else(|| format!("missing part {name}"))?;
    let s = String::from_utf8(std::mem::take(bytes))?;
    *bytes = f(&s).into_bytes();
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

fn fnv1a(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn unzip(bytes: &[u8]) -> Res<Vec<(String, Vec<u8>)>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut parts = Vec::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        let mut b = Vec::new();
        f.read_to_end(&mut b)?;
        parts.push((f.name().to_string(), b));
    }
    Ok(parts)
}

fn zip(parts: &[(String, Vec<u8>)]) -> Res<Vec<u8>> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        z.start_file(name, zip::write::SimpleFileOptions::default())?;
        z.write_all(bytes)?;
    }
    Ok(z.finish()?.into_inner())
}

// ---------------------------------------------------------------- check

#[derive(Default)]
struct Bookmark {
    id: String,
    name: String,
    start: Option<usize>,
    end: Option<usize>,
}

#[derive(Default)]
struct Sdt {
    tag: String,
    id: String,
    appearance: String,
    start: Option<usize>,
    end: Option<usize>,
}

/// A run carrying `mg:` attributes (C) or our character style (E).
struct MarkedRun {
    key: String,
    start: usize,
    end: usize,
}

#[derive(Default)]
struct Body {
    /// Plain text of the body, paragraphs separated by `\n`.
    text: String,
    pos: usize,
    bookmarks: Vec<Bookmark>,
    sdts: Vec<Sdt>,
    attr_runs: Vec<MarkedRun>,
    style_runs: Vec<MarkedRun>,
    root: String,
    issues: Vec<String>,
}

impl Body {
    fn slice(&self, start: usize, end: usize) -> String {
        self.text.chars().skip(start).take(end.saturating_sub(start)).collect()
    }
}

fn check(file: &str) -> Res<()> {
    let bytes = std::fs::read(file)?;
    let parts = unzip(&bytes)?;
    let names: Vec<&str> = parts.iter().map(|(n, _)| n.as_str()).collect();
    let mut out = String::new();
    let mut issues: Vec<String> = Vec::new();

    for (name, b) in &parts {
        if (name.ends_with(".xml") || name.ends_with(".rels"))
            && let Err(e) = well_formed(b)
        {
            issues.push(format!("malformed {name}: {e}"));
        }
    }
    content_types(&parts, &names, &mut issues)?;
    for (name, b) in parts.iter().filter(|(n, _)| n.ends_with(".rels")) {
        rels_targets(name, b, &names, &mut issues)?;
    }

    let doc = parts.iter().find(|(n, _)| n == "word/document.xml").ok_or("no word/document.xml")?;
    let body = walk_body(&doc.1)?;
    issues.extend(body.issues.iter().cloned());

    let _ = writeln!(out, "root {}", body.root);
    let paras: Vec<&str> = body.text.split('\n').collect();
    let _ = writeln!(out, "paragraphs {}", paras.len().saturating_sub(1));
    for (i, p) in paras.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
        let _ = writeln!(out, "p{i} {p:?}");
    }

    let _ = writeln!(out, "bookmarks {}", body.bookmarks.len());
    for b in &body.bookmarks {
        match (b.start, b.end) {
            (Some(s), Some(e)) if e >= s => {
                let _ = writeln!(out, "bookmark {} id={} text={:?}", b.name, b.id, body.slice(s, e));
            }
            (Some(_), Some(_)) => {
                let _ = writeln!(out, "bookmark {} id={} END-BEFORE-START", b.name, b.id);
                issues.push(format!("bookmark-end-before-start {}", b.name));
            }
            (Some(_), None) => {
                let _ = writeln!(out, "bookmark {} id={} NO-END", b.name, b.id);
                issues.push(format!("orphan-bookmark-start {}", b.name));
            }
            _ => {}
        }
    }
    let _ = writeln!(out, "sdt {}", body.sdts.len());
    for c in &body.sdts {
        let text = match (c.start, c.end) {
            (Some(s), Some(e)) => body.slice(s, e),
            _ => String::new(),
        };
        let _ = writeln!(out, "sdt id={} appearance={:?} tag={:?} text={text:?}", c.id, c.appearance, c.tag);
    }
    let attr_groups = groups(&body.attr_runs);
    let _ = writeln!(out, "ignorable-groups {}", attr_groups.len());
    for (key, s, e) in &attr_groups {
        let _ = writeln!(out, "ignorable {key} text={:?}", body.slice(*s, *e));
    }
    let style_groups = groups(&body.style_runs);
    let _ = writeln!(out, "styled-groups {}", style_groups.len());
    for (key, s, e) in &style_groups {
        let _ = writeln!(out, "styled {key} text={:?}", body.slice(*s, *e));
    }

    // Our custom XML part(s), and whether each span still finds its text.
    let mut spans = 0;
    let mut resolved = 0;
    let mut matched = 0;
    for (name, b) in &parts {
        let lower = name.to_ascii_lowercase();
        if !(lower.starts_with("customxml/item") && lower.ends_with(".xml")) || lower.contains("itemprops") {
            continue;
        }
        let Some(list) = margin_spans(b)? else { continue };
        let related = parts
            .iter()
            .filter(|(n, _)| n == "word/_rels/document.xml.rels" || n == "_rels/.rels")
            .any(|(_, r)| String::from_utf8_lossy(r).contains(&name[name.rfind('/').map_or(0, |i| i + 1)..]));
        let _ = writeln!(out, "customxml {name} spans={} related={}", list.len(), if related { "yes" } else { "no" });
        for a in &list {
            spans += 1;
            let get = |k: &str| a.get(k).map(String::as_str).unwrap_or("");
            let found: Option<String> = match get("anchor") {
                "bookmark" => body.bookmarks.iter().find(|b| b.name == get("ref")).and_then(|b| Some(body.slice(b.start?, b.end?))),
                "offset" => {
                    let (p, s, e) = (get("para").parse::<usize>().ok(), get("start").parse::<usize>().ok(), get("end").parse::<usize>().ok());
                    match (p.and_then(|p| paras.get(p)), s, e) {
                        (Some(text), Some(s), Some(e)) => Some(text.chars().skip(s).take(e.saturating_sub(s)).collect()),
                        _ => None,
                    }
                }
                "style" => style_groups.iter().map(|(_, s, e)| body.slice(*s, *e)).find(|t| fnv1a(t) == get("hash")),
                _ => None,
            };
            let ok = found.as_deref().is_some_and(|t| fnv1a(t) == get("hash"));
            resolved += usize::from(found.is_some());
            matched += usize::from(ok);
            let _ = writeln!(
                out,
                "span anchor={} ref={} author={} acceptedAt={} resolved={} text-match={} text={:?}",
                get("anchor"),
                get("ref"),
                get("author"),
                get("acceptedAt"),
                if found.is_some() { "yes" } else { "no" },
                if ok { "yes" } else { "no" },
                found.unwrap_or_default()
            );
        }
    }
    for i in &issues {
        let _ = writeln!(out, "issue {i}");
    }
    let _ = writeln!(
        out,
        "summary bookmarks={} sdt={} ignorable-groups={} styled-groups={} spans={spans} resolved={resolved} matched={matched} issues={}",
        body.bookmarks.iter().filter(|b| b.start.is_some() && b.end.is_some()).count(),
        body.sdts.len(),
        attr_groups.len(),
        style_groups.len(),
        issues.len()
    );
    print!("{out}");
    Ok(())
}

/// Merge adjacent runs with the same key into (key, start, end) groups.
fn groups(runs: &[MarkedRun]) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = Vec::new();
    for r in runs {
        match out.last_mut() {
            Some((k, _, e)) if *k == r.key && *e == r.start => *e = r.end,
            _ => out.push((r.key.clone(), r.start, r.end)),
        }
    }
    out
}

fn well_formed(bytes: &[u8]) -> Res<()> {
    let mut r = NsReader::from_reader(bytes);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if let (_, Event::Eof) = r.read_resolved_event_into(&mut buf)? {
            return Ok(());
        }
    }
}

/// Every part has a content type (Default by extension or Override by name).
fn content_types(parts: &[(String, Vec<u8>)], names: &[&str], issues: &mut Vec<String>) -> Res<()> {
    let Some((_, ct)) = parts.iter().find(|(n, _)| n == "[Content_Types].xml") else {
        issues.push("no [Content_Types].xml".into());
        return Ok(());
    };
    let mut defaults: BTreeMap<String, String> = BTreeMap::new();
    let mut overrides: BTreeMap<String, String> = BTreeMap::new();
    let mut r = NsReader::from_reader(&ct[..]);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let (ns, ev) = r.read_resolved_event_into(&mut buf)?;
        match ev {
            Event::Start(e) | Event::Empty(e) if ns_is(&ns, CT_NS) => {
                let a = attrs(&e);
                match e.local_name().as_ref() {
                    b"Default" => {
                        defaults.insert(
                            a.get("Extension").cloned().unwrap_or_default().to_ascii_lowercase(),
                            a.get("ContentType").cloned().unwrap_or_default(),
                        );
                    }
                    b"Override" => {
                        overrides.insert(
                            a.get("PartName").cloned().unwrap_or_default().to_ascii_lowercase(),
                            a.get("ContentType").cloned().unwrap_or_default(),
                        );
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    for n in names.iter().filter(|n| **n != "[Content_Types].xml" && !n.ends_with('/')) {
        let ext = n.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if !overrides.contains_key(&format!("/{}", n.to_ascii_lowercase())) && !defaults.contains_key(&ext) {
            issues.push(format!("content-type-missing {n}"));
        }
    }
    Ok(())
}

/// Every internal relationship target of a `.rels` part exists in the package.
fn rels_targets(rels_name: &str, bytes: &[u8], names: &[&str], issues: &mut Vec<String>) -> Res<()> {
    // `x/_rels/y.rels` describes part `x/y`; targets are relative to `x/`.
    let base = match rels_name.rsplit_once("_rels/") {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    };
    let mut r = NsReader::from_reader(bytes);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let (ns, ev) = r.read_resolved_event_into(&mut buf)?;
        match ev {
            Event::Start(e) | Event::Empty(e) if ns_is(&ns, REL_NS) && e.local_name().as_ref() == b"Relationship" => {
                let a = attrs(&e);
                if a.get("TargetMode").is_some_and(|m| m == "External") {
                    continue;
                }
                let target = a.get("Target").cloned().unwrap_or_default();
                let full = resolve_path(&base, &target);
                if !names.iter().any(|n| n.eq_ignore_ascii_case(&full)) {
                    issues.push(format!("missing-rel-target {rels_name} -> {target}"));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

fn resolve_path(base: &str, target: &str) -> String {
    let mut segs: Vec<&str> = if target.starts_with('/') { Vec::new() } else { base.split('/').filter(|s| !s.is_empty()).collect() };
    for s in target.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    segs.join("/")
}

fn ns_is(ns: &ResolveResult<'_>, uri: &str) -> bool {
    matches!(ns, ResolveResult::Bound(n) if n.as_ref() == uri.as_bytes())
}

/// Attributes by local name (prefix-insensitive), values unescaped.
fn attrs(e: &quick_xml::events::BytesStart<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for a in e.attributes().with_checks(false).flatten() {
        let key = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
        let val =
            a.normalized_value(XmlVersion::Explicit1_0).map(|v| v.into_owned()).unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
        out.insert(key, val);
    }
    out
}

/// Walk `word/document.xml`: body text with positions, bookmarks, content controls, runs with
/// `mg:` attributes, runs in our character style.
fn walk_body(bytes: &[u8]) -> Res<Body> {
    let mut b = Body::default();
    let mut r = NsReader::from_reader(bytes);
    let mut buf = Vec::new();
    let mut in_t = false;
    let mut sdt_stack: Vec<Sdt> = Vec::new();
    let mut in_sdt_pr = 0usize;
    let mut run_start: Option<usize> = None;
    let mut run_key: Option<String> = None;
    let mut run_style: Option<String> = None;
    let mut seen_root = false;
    loop {
        buf.clear();
        let (ns, ev) = r.read_resolved_event_into(&mut buf)?;
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(ev, Event::Empty(_));
                if !seen_root {
                    seen_root = true;
                    let mut a: Vec<String> = Vec::new();
                    for at in e.attributes().with_checks(false).flatten() {
                        let k = String::from_utf8_lossy(at.key.as_ref()).into_owned();
                        if k == "mc:Ignorable" || k == "mc:PreserveAttributes" || k == "xmlns:mg" || at.value.as_ref() == MG_NS.as_bytes() {
                            a.push(format!("{k}={:?}", String::from_utf8_lossy(&at.value)));
                        }
                    }
                    b.root = a.join(" ");
                }
                let w = ns_is(&ns, W_NS);
                let local_name = e.local_name();
                let local = local_name.as_ref();
                if w {
                    match local {
                        b"t" if !empty => in_t = true,
                        b"bookmarkStart" => {
                            let a = attrs(e);
                            let id = a.get("id").cloned().unwrap_or_default();
                            if b.bookmarks.iter().any(|x| x.id == id) {
                                b.issues.push(format!("duplicate-bookmark-id {id}"));
                            }
                            b.bookmarks.push(Bookmark { id, name: a.get("name").cloned().unwrap_or_default(), start: Some(b.pos), end: None });
                        }
                        b"bookmarkEnd" => {
                            let id = attrs(e).get("id").cloned().unwrap_or_default();
                            match b.bookmarks.iter_mut().find(|x| x.id == id && x.end.is_none()) {
                                Some(x) => x.end = Some(b.pos),
                                None => b.issues.push(format!("orphan-bookmark-end {id}")),
                            }
                        }
                        b"sdt" if !empty => sdt_stack.push(Sdt::default()),
                        b"sdtPr" if !empty => in_sdt_pr += 1,
                        b"sdtContent" if !empty => {
                            if let Some(c) = sdt_stack.last_mut() {
                                c.start = Some(b.pos);
                            }
                        }
                        b"tag" if in_sdt_pr > 0 => {
                            if let Some(c) = sdt_stack.last_mut() {
                                c.tag = attrs(e).get("val").cloned().unwrap_or_default();
                            }
                        }
                        b"id" if in_sdt_pr > 0 => {
                            if let Some(c) = sdt_stack.last_mut() {
                                c.id = attrs(e).get("val").cloned().unwrap_or_default();
                            }
                        }
                        b"r" if !empty => {
                            run_start = Some(b.pos);
                            run_style = None;
                            let mut mg: Vec<String> = Vec::new();
                            for a in e.attributes().with_checks(false).flatten() {
                                let (ans, alocal) = r.resolver_mut().resolve_attribute(a.key);
                                if ns_is(&ans, MG_NS) {
                                    let v = a.normalized_value(XmlVersion::Explicit1_0).map(|v| v.into_owned()).unwrap_or_default();
                                    mg.push(format!("{}={v}", String::from_utf8_lossy(alocal.as_ref())));
                                }
                            }
                            run_key = (!mg.is_empty()).then(|| mg.join(";"));
                        }
                        b"rStyle" => {
                            if let Some(v) = attrs(e).get("val").filter(|v| v.starts_with(STYLE_ID)) {
                                run_style = Some(v.clone());
                            }
                        }
                        _ => {}
                    }
                } else if ns_is(&ns, W15_NS)
                    && local == b"appearance"
                    && in_sdt_pr > 0
                    && let Some(c) = sdt_stack.last_mut()
                {
                    c.appearance = attrs(e).get("val").cloned().unwrap_or_default();
                }
            }
            Event::End(ref e) => {
                let w = ns_is(&ns, W_NS);
                if !w {
                    continue;
                }
                match e.local_name().as_ref() {
                    b"t" => in_t = false,
                    b"p" => {
                        b.text.push('\n');
                        b.pos += 1;
                    }
                    b"sdtPr" => in_sdt_pr = in_sdt_pr.saturating_sub(1),
                    b"sdtContent" => {
                        if let Some(c) = sdt_stack.last_mut() {
                            c.end = Some(b.pos);
                        }
                    }
                    b"sdt" => {
                        if let Some(c) = sdt_stack.pop() {
                            b.sdts.push(c);
                        }
                    }
                    b"r" => {
                        if let (Some(s), Some(k)) = (run_start, run_key.take()) {
                            b.attr_runs.push(MarkedRun { key: k, start: s, end: b.pos });
                        }
                        if let (Some(s), Some(k)) = (run_start, run_style.take()) {
                            b.style_runs.push(MarkedRun { key: k, start: s, end: b.pos });
                        }
                        run_start = None;
                    }
                    _ => {}
                }
            }
            Event::Text(ref t) if in_t => {
                let s = t.decode()?;
                b.pos += s.chars().count();
                b.text.push_str(&s);
            }
            Event::CData(ref t) if in_t => {
                let s = String::from_utf8_lossy(t.as_ref()).into_owned();
                b.pos += s.chars().count();
                b.text.push_str(&s);
            }
            Event::GeneralRef(ref g) if in_t => {
                let c = match g.resolve_char_ref()? {
                    Some(c) => Some(c),
                    None => match g.decode()?.as_ref() {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        _ => None,
                    },
                };
                if let Some(c) = c {
                    b.pos += 1;
                    b.text.push(c);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    for c in sdt_stack {
        b.issues.push(format!("unclosed-sdt {}", c.id));
    }
    Ok(b)
}

/// The `mg:span` entries of a custom XML part whose root is ours; `None` for another store.
fn margin_spans(bytes: &[u8]) -> Res<Option<Vec<BTreeMap<String, String>>>> {
    let mut r = NsReader::from_reader(bytes);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut root_seen = false;
    loop {
        buf.clear();
        let (ns, ev) = r.read_resolved_event_into(&mut buf)?;
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if !root_seen {
                    root_seen = true;
                    if !ns_is(&ns, MG_NS) {
                        return Ok(None);
                    }
                    continue;
                }
                if ns_is(&ns, MG_NS) && e.local_name().as_ref() == b"span" {
                    out.push(attrs(e));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(Some(out))
}
