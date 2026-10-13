//! Margin's authorship part: `customXml/itemN.xml` in the namespace `urn:margin:authorship`
//! (ECMA-376 Part 1 §11.3.5 custom XML data storage, with its `itemPropsN.xml`), one `mg:span`
//! per `_MarginAI_` bookmark (`docs/authorship-storage.md`). The part is rewritten from the
//! model on every save and only exists while the document has accepted assistant text; nothing
//! in it is a prompt, a key or a path.
//!
//! ```xml
//! <mg:authorship xmlns:mg="urn:margin:authorship" mg:version="1">
//!   <mg:span mg:anchor="bookmark" mg:ref="_MarginAI_1a2b3c4d" mg:author="Claude" mg:kind="ai"
//!            mg:acceptedAt="2026-10-13T09:00:00Z" mg:model="claude-opus-5" mg:len="33"
//!            mg:hash="…"><mg:text>the accepted text</mg:text></mg:span>
//! </mg:authorship>
//! ```

use wordcraft_doc::Document;
use wordcraft_doc::authorship::{MAX_SPANS, MAX_STORED_TEXT_CHARS, NAMESPACE, Span, is_mark, scan};

use crate::xml::{W, parse_ns};

/// The data store item id (`ds:itemID`); fixed, so a second save writes the same bytes.
pub const ITEM_ID: &str = "{6D1F3B2A-0C4E-4F8B-9A7D-2E5C1B3F4A60}";
/// Longest part looked at (bytes); a bigger one is left alone.
const MAX_PART_BYTES: usize = 64 * 1024 * 1024;

/// The spans of a custom XML part that is ours; `None` for another store (or one that doesn't
/// parse). Entries that cannot be trusted (no mark name, no text and no hash) are skipped.
pub fn read_part(bytes: &[u8]) -> Option<Vec<Span>> {
    if bytes.len() > MAX_PART_BYTES {
        return None;
    }
    let root = parse_ns(bytes, &[("mg", NAMESPACE)]).ok()?;
    if root.name != "mg:authorship" {
        return None;
    }
    let mut out = Vec::new();
    for el in root.els().filter(|e| e.name == "mg:span").take(MAX_SPANS) {
        let Some(name) = el.attr("mg:ref").map(str::trim).filter(|n| is_mark(n)) else { continue };
        let author = el.attr("mg:author").unwrap_or("");
        let model = el.attr("mg:model").unwrap_or("");
        let at = el.attr("mg:acceptedAt").unwrap_or("");
        let text = el.els().find(|t| t.name == "mg:text").map(|t| t.text());
        let span = match text {
            Some(t) if t.chars().count() <= MAX_STORED_TEXT_CHARS => Span::new(name, author, model, at, &t),
            _ => {
                let Some(hash) = el.attr("mg:hash").filter(|h| h.len() == 16 && h.bytes().all(|b| b.is_ascii_hexdigit())) else { continue };
                let len = el.attr("mg:len").and_then(|l| l.parse::<usize>().ok()).unwrap_or(0);
                Span::hash_only(name, author, model, at, hash, len)
            }
        };
        out.push(span);
    }
    Some(out)
}

/// The part for `doc`: one span per `_MarginAI_` bookmark that has an entry, in document order.
/// `None` when the document has none (then no part is written).
pub fn part_xml(doc: &Document) -> Option<Vec<u8>> {
    let mut w = W::new();
    w.open("mg:authorship", &[("xmlns:mg", NAMESPACE), ("mg:version", "1")]);
    let mut n = 0usize;
    for r in scan(doc).ranges {
        let Some(s) = doc.authorship.get(&r.name) else { continue };
        n += 1;
        let (len, hash) = (s.len.to_string(), s.hash.as_str());
        let attrs = [
            ("mg:anchor", "bookmark"),
            ("mg:ref", s.name.as_str()),
            ("mg:author", s.author.as_str()),
            ("mg:kind", "ai"),
            ("mg:acceptedAt", s.accepted_at.as_str()),
            ("mg:model", s.model.as_str()),
            ("mg:len", len.as_str()),
            ("mg:hash", hash),
        ];
        match &s.text {
            Some(t) => {
                w.open("mg:span", &attrs);
                w.leaf("mg:text", &[], t);
                w.close("mg:span");
            }
            None => w.empty("mg:span", &attrs),
        }
    }
    w.close("mg:authorship");
    (n > 0).then(|| w.into_bytes())
}

/// The part's `itemPropsN.xml`.
pub fn props_xml() -> Vec<u8> {
    let mut w = W::new();
    w.open("ds:datastoreItem", &[("xmlns:ds", "http://schemas.openxmlformats.org/officeDocument/2006/customXml"), ("ds:itemID", ITEM_ID)]);
    w.open("ds:schemaRefs", &[]);
    w.empty("ds:schemaRef", &[("ds:uri", NAMESPACE)]);
    w.close("ds:schemaRefs");
    w.close("ds:datastoreItem");
    w.into_bytes()
}
