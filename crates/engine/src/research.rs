//! O8 Research (OPTIMO.md §9, rule 4): where findings come from, and how a source is written.
//!
//! * **Files.** With folders allowed in Settings › Co-writer, the files whose text shares the
//!   most words with the question are sent with numbered lines ([`files`]); a finding cites
//!   `[file name:line]`, and the reader checks that the file was sent, that the line exists and
//!   that it says what the finding says ([`supports`]).
//! * **The web,** through the model provider's own search tool (Mistral's `web_search`, the same
//!   key). The UI turns the provider's answer into an [`envelope`]: the answer text with a marker
//!   where the provider attached a reference, the references, and the pages the search returned.
//!   A web finding's source is a reference the provider attached, never an address the model
//!   wrote; it must be a page the search returned.
//!
//! The model never answers from memory as if that were a source: a line without a checked file
//! line or a fetched page is refused.

use serde_json::{Value, json};

use crate::Session;
use crate::assistant_edits::{self as edits, ScopeKind};
use crate::assistant_source::{self as source, SourceText};
use crate::cowriter::Source;

/// Files opened to pick from, and sent.
const MAX_OPENED: usize = 12;
pub const MAX_FILES: usize = 4;
/// Pages and references kept from one web answer.
pub const MAX_PAGES: usize = 40;

/// A reference marker in an [`envelope`]'s text: `REF_OPEN` + index + `REF_CLOSE`. Private-use
/// characters, taken out of the model's own text first so it can't forge one.
pub const REF_OPEN: char = '\u{E000}';
pub const REF_CLOSE: char = '\u{E001}';

/// A web page: a reference the provider attached, or a page its search returned.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub title: String,
    pub url: String,
    /// The page's date as the search gave it ("2024-06-01", "1903"…), or empty.
    pub date: String,
}

/// A piece of a web answer, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Cite(Page),
}

fn private(c: char) -> bool {
    ('\u{E000}'..='\u{F8FF}').contains(&c)
}

/// The words of `text` that carry meaning: 4+ letters or any number, lower-case.
pub fn key_words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4 || (!w.is_empty() && w.chars().all(|c| c.is_ascii_digit())))
        .map(str::to_lowercase)
        .collect();
    out.sort();
    out.dedup();
    out.truncate(400);
    out
}

/// What the UI hands the reader for a web run: `{"optimo_web": 1, "text", "refs", "searched"}`.
pub fn envelope(pieces: &[Piece], searched: &[Page]) -> String {
    let mut text = String::new();
    let mut refs: Vec<Value> = Vec::new();
    for p in pieces {
        match p {
            Piece::Text(t) => text.extend(t.chars().filter(|c| !private(*c))),
            Piece::Cite(page) if refs.len() < MAX_PAGES => {
                text.push(REF_OPEN);
                text.push_str(&refs.len().to_string());
                text.push(REF_CLOSE);
                refs.push(json!({"title": page.title, "url": page.url, "date": page.date}));
            }
            Piece::Cite(_) => {}
        }
    }
    let searched: Vec<Value> = searched.iter().take(MAX_PAGES).map(|p| json!({"title": p.title, "url": p.url, "date": p.date})).collect();
    json!({"optimo_web": 1, "text": text, "refs": refs, "searched": searched}).to_string()
}

/// A web answer read back from an [`envelope`].
pub struct WebAnswer {
    pub text: String,
    pub refs: Vec<Page>,
    pub searched: Vec<Page>,
}

fn pages(v: Option<&Value>) -> Vec<Page> {
    let s = |p: &Value, k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().chars().take(300).collect::<String>();
    v.and_then(Value::as_array)
        .map(|a| a.iter().take(MAX_PAGES).map(|p| Page { title: s(p, "title"), url: s(p, "url"), date: s(p, "date") }).collect())
        .unwrap_or_default()
}

pub fn read_envelope(text: &str) -> Option<WebAnswer> {
    let v: Value = serde_json::from_str(text).ok()?;
    v.get("optimo_web")?;
    Some(WebAnswer { text: v.get("text")?.as_str()?.to_string(), refs: pages(v.get("refs")), searched: pages(v.get("searched")) })
}

/// A real web address: http(s), a host with a dot, no spaces. Lower-cased host, no trailing
/// slash or fragment, for comparing.
pub fn web_address(url: &str) -> Option<String> {
    let u = url.trim();
    if u.chars().count() > 2048 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let (scheme, rest) = u.split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return None;
    }
    let (host, path) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h).to_ascii_lowercase();
    let bare = host.split(':').next().unwrap_or_default();
    let labels: Vec<&str> = bare.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|l| l.is_empty() || !l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return None;
    }
    let path = path.split('#').next().unwrap_or_default().trim_end_matches('/');
    Some(format!("{}://{host}{path}", scheme.to_ascii_lowercase()))
}

/// The site a page is on: its host without `www.`.
pub fn site(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.trim_start_matches("www.").to_ascii_lowercase()
}

/// The first year (1500–2099) in `date`, or empty.
pub fn year(date: &str) -> String {
    let d: Vec<char> = date.chars().collect();
    for w in d.windows(4).enumerate() {
        let (i, w) = w;
        let before = i.checked_sub(1).and_then(|j| d.get(j)).is_some_and(char::is_ascii_digit);
        let after = d.get(i + 4).is_some_and(char::is_ascii_digit);
        let s: String = w.iter().collect();
        if !before && !after && s.parse::<u32>().is_ok_and(|y| (1500..2100).contains(&y)) {
            return s;
        }
    }
    String::new()
}

/// A web page as a finding's source: title · site · year.
pub fn web_source(p: &Page) -> Source {
    let site = site(&p.url);
    let year = year(&p.date);
    let detail = if year.is_empty() { site.clone() } else { format!("{site} · {year}") };
    let title = if p.title.trim().is_empty() { site } else { p.title.trim().chars().take(160).collect() };
    Source { title, url: p.url.trim().to_string(), detail }
}

/// A file line as a finding's source.
pub fn file_source(name: &str, line: usize) -> Source {
    Source { title: name.to_string(), url: String::new(), detail: format!("line {line} · your files") }
}

/// The source as a footnote or quote writes it: "Title, site, year, address" / "file, line 12".
pub fn citation(src: &Source) -> String {
    let mut parts = vec![src.title.trim().to_string()];
    if src.url.is_empty() {
        if let Some(line) = src.detail.split(" · ").next().filter(|l| l.starts_with("line ")) {
            parts.push(line.to_string());
        }
    } else {
        parts.extend(src.detail.split(" · ").map(str::to_string).filter(|d| !d.is_empty() && *d != src.title));
        parts.push(src.url.trim().to_string());
    }
    parts.retain(|p| !p.is_empty());
    parts.join(", ")
}

/// The allowed files that share the most words with `query`, within the source budget.
// ponytail: opens the first MAX_OPENED listed files on the calling thread, like Check; an index
// of the folders would let a large folder be searched.
pub fn files(folders: &[String], query: &str) -> Vec<SourceText> {
    if folders.is_empty() {
        return Vec::new();
    }
    let words = key_words(query);
    let mut read: Vec<(usize, SourceText)> = Vec::new();
    for p in source::list(folders).into_iter().take(MAX_OPENED) {
        if let Ok(s) = source::read(&p.to_string_lossy(), folders) {
            let lower = s.text.to_lowercase();
            let score = words.iter().filter(|w| lower.contains(w.as_str())).count();
            if score > 0 {
                read.push((score, s));
            }
        }
    }
    // Most matching first; ties by name, so the reader picks the same files as the prompt.
    read.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    let mut budget = source::MAX_SOURCE_CHARS;
    let mut out = Vec::new();
    for (_, mut s) in read.into_iter().take(MAX_FILES) {
        if budget < 500 {
            break;
        }
        let n = s.text.chars().count();
        if n > budget {
            s.text = s.text.chars().take(budget).collect();
            s.trimmed = true;
        }
        budget = budget.saturating_sub(n);
        out.push(s);
    }
    out
}

/// A file's text with numbered lines, as it is sent ("12| …"). Empty lines keep their number.
pub fn numbered(text: &str) -> String {
    text.lines().enumerate().map(|(i, l)| format!("{}| {l}", i + 1)).collect::<Vec<_>>().join("\n")
}

/// Whether line `line` (1-based) of `text`, with its neighbours, says what `finding` says: it
/// exists and holds at least two of the finding's meaning words (one when the finding has one).
pub fn supports(text: &str, line: usize, finding: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    let Some(i) = line.checked_sub(1).filter(|i| *i < lines.len()) else { return false };
    // Whole words: "90" is not in "1903".
    let near = key_words(&lines.get(i.saturating_sub(1)..(i + 2).min(lines.len())).unwrap_or_default().join(" "));
    let words = key_words(finding);
    let hits = words.iter().filter(|w| near.contains(w)).count();
    hits >= words.len().clamp(1, 2)
}

/// Where a footnote goes: the end of the selection (its last paragraph), else the end of the
/// caret's paragraph, else the last paragraph with text before it. The quote returned is the
/// shortest tail of that text (6 words, 12, all) found exactly once in the document.
pub fn anchor(s: &Session) -> Option<String> {
    let doc = edits::scope(s, ScopeKind::Document).ok()?;
    let sel = edits::scope(s, ScopeKind::Selection).ok().filter(|_| !s.sel.is_collapsed());
    let text = match sel {
        Some(sc) => sc.text.lines().rev().find(|l| !l.trim().is_empty())?.to_string(),
        None => {
            let here = s.sel.focus.clone();
            let mut best = None;
            for p in &doc.pieces {
                let line = s.doc.para(here.story, &p.path).map(|x| x.final_text()).unwrap_or_default();
                if !line.trim().is_empty() {
                    best = Some(line);
                }
                if p.path == here.path {
                    break;
                }
            }
            best?
        }
    };
    let text = text.trim_end();
    let words: Vec<&str> = text.split_whitespace().collect();
    for n in [6usize, 12, usize::MAX] {
        let take = n.min(words.len());
        let tail = words.get(words.len() - take..).unwrap_or_default().join(" ");
        // The tail as it reads in the text (the words joined by the text's own spaces).
        let Some(start) = text.rfind(words.get(words.len() - take).copied().unwrap_or_default()) else { continue };
        let quote = text.get(start..).unwrap_or_default();
        if !quote.is_empty() && quote.split_whitespace().collect::<Vec<_>>().join(" ") == tail && edits::locate(s, &doc, quote).is_ok() {
            return Some(quote.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_doc::Document;

    use crate::Session;
    use crate::assistant_edits::{ScopeKind, scope};
    use crate::cowriter::{self, Call, Output, Reply, Verb};

    const NAME: &str = "AI · Mistral";

    fn session() -> Session {
        let mut s = Session::new(Document::new());
        s.run("text.insert", &json!({"text": "The Albula line crosses the Alps. It has many bridges.", "raw": true})).unwrap();
        s
    }

    fn ran(s: &Session, opts: &serde_json::Value, answer: &str) -> cowriter::Ran {
        let sc = scope(s, ScopeKind::Document).unwrap();
        cowriter::run(
            Verb::Research,
            &Call { session: s, scope: &sc, options: opts, name: NAME },
            Some(Reply { text: answer, stop_reason: Some("stop") }),
        )
        .unwrap()
    }

    #[test]
    fn file_findings_need_a_line_that_says_them() {
        let s = session();
        let base = std::env::temp_dir().join(format!("wc-research-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("albula.txt"), "# Albula\nThe Albula line opened on 1 July 1903.\nIt has 55 bridges and 39 tunnels.").unwrap();
        std::fs::write(base.join("recipes.txt"), "flour, water, salt").unwrap();
        let opts = json!({"query": "Albula line bridges", "anchor": "many bridges.", "folders": [base.to_string_lossy()]});
        let sc = scope(&s, ScopeKind::Document).unwrap();
        let p = cowriter::prompt(Verb::Research, &Call { session: &s, scope: &sc, options: &opts, name: NAME }).unwrap().unwrap();
        assert!(p.user.contains("albula.txt") && p.user.contains("3| It has 55 bridges"), "{}", p.user);
        assert!(!p.user.contains("flour"), "a file without the question's words is not sent");
        let answer = "Summary: One file.\n- The line has 55 bridges and 39 tunnels [albula.txt:3].\n- The line has 90 viaducts [albula.txt:3].\n- It opened in 1903 [albula.txt:42].\n- Bridges abound [recipes.txt:1].";
        let out = ran(&s, &opts, answer);
        assert_eq!(out.outputs.len(), 1, "{:?}", out.lines);
        let Output::Footnote { text, source, .. } = &out.outputs[0] else { panic!() };
        assert_eq!(text, "The line has 55 bridges and 39 tunnels.");
        assert_eq!(source.title, "albula.txt");
        assert_eq!(source.detail, "line 3 · your files");
        assert_eq!(out.lines.len(), 3, "{:?}", out.lines);
        let _ = std::fs::remove_dir_all(&base);
    }
}
