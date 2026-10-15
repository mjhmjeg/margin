//! A source file for "Check facts against a file…" (HANDOFF §6 K4, step A4): read-only text from
//! a file inside the folders the person allowed in Settings › Assistant.
//!
//! * **Folders.** The same check as the MCP sandbox's read root ([`crate::file_access`]), with
//!   the person's list as the roots: the path is made absolute, its links are followed, and it
//!   must lie inside one of the folders (whole folder names, so `/work2` is not inside `/work`).
//!   `..` and links that lead out are refused. Nothing is ever written.
//! * **Files.** Regular files only, at most [`MAX_SOURCE_BYTES`], of a type Optimo reads as text
//!   ([`EXTS`]). The file is read through the checked gateway (`io::read_file_capped`) from the
//!   resolved path, then parsed by the same readers as File › Open.
//! * **Cap.** At most [`MAX_SOURCE_CHARS`] characters go to the model, cut at a paragraph end
//!   (the person is told, [`SourceText::trimmed`]).
//!
//! A path is checked, then read: another program that swaps a link in between can still win that
//! race (see `docs/mcp.md`).

use std::path::PathBuf;

use crate::file_access::{self, AutomationRoots};

/// Most characters of a source file sent with one ask (about 10,000 tokens).
pub const MAX_SOURCE_CHARS: usize = 40_000;
/// Largest file read at all.
pub const MAX_SOURCE_BYTES: u64 = 20 << 20;
/// Most files listed for the person to pick from.
pub const MAX_LISTED: usize = 300;
/// How many folders below an allowed folder are listed.
pub const LIST_DEPTH: usize = 3;
/// File types read as text. PDF is not among them: WordCraft has no PDF text extraction.
pub const EXTS: [&str; 10] = ["txt", "text", "md", "markdown", "docx", "docm", "odt", "rtf", "html", "htm"];

/// The text of a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceText {
    /// The file's name (no folders), cleaned for the prompt.
    pub name: String,
    pub text: String,
    /// The file has more text than [`MAX_SOURCE_CHARS`]; `text` is its beginning.
    pub trimmed: bool,
    /// Characters in the whole file's text.
    pub total_chars: usize,
}

impl SourceText {
    pub fn words(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

/// The files in `folders` the person can pick from, sorted.
pub fn list(folders: &[String]) -> Vec<PathBuf> {
    let (roots, _) = AutomationRoots::for_app(folders);
    roots.list_files(&EXTS, LIST_DEPTH, MAX_LISTED)
}

fn clean_name(path: &std::path::Path) -> String {
    let n: String = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    n.chars().filter(|c| !c.is_control()).take(80).collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_bytes(roots: &std::sync::Arc<AutomationRoots>, path: &str) -> Result<Vec<u8>, String> {
    file_access::confine(Some(roots), || crate::io::read_file_capped(path, MAX_SOURCE_BYTES, "file"))
}

#[cfg(target_arch = "wasm32")]
fn read_bytes(_roots: &std::sync::Arc<AutomationRoots>, path: &str) -> Result<Vec<u8>, String> {
    Err(format!("{path}: the assistant reads files in the desktop app"))
}

/// Read the file at `path`, which must lie inside `folders`.
pub fn read(path: &str, folders: &[String]) -> Result<SourceText, String> {
    if folders.is_empty() {
        return Err("no folders are allowed: add one in Settings › Assistant first".into());
    }
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !EXTS.contains(&ext.as_str()) {
        let shown = if ext.is_empty() { "files without a type".to_string() } else { format!(".{ext} files") };
        return Err(format!("can't read {shown}: use one of {}", EXTS.map(|e| format!(".{e}")).join(" ")));
    }
    let (roots, _) = AutomationRoots::for_app(folders);
    let resolved = roots.resolve_read(path)?;
    // A link named `.txt` that leads to another kind of file is that kind of file.
    let real = resolved.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !EXTS.contains(&real.as_str()) {
        return Err(format!("can't read that file: it is a .{real} file; use one of {}", EXTS.map(|e| format!(".{e}")).join(" ")));
    }
    let name = clean_name(&resolved);
    let shown = resolved.to_string_lossy().into_owned();
    // Checked a second time inside the gateway every file read goes through.
    let bytes = read_bytes(&roots, &shown)?;
    // A damaged file must not take the app down with it.
    let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::io::open_bytes_with(&shown, &bytes, None).map(|(doc, _)| doc.plain_text(wordcraft_doc::StoryRef::Body))
    }));
    let full = match parsed {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => return Err(format!("{name}: {e}")),
        Err(_) => return Err(format!("{name}: the file could not be read")),
    };
    let full: String = full.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t')).collect();
    if full.trim().is_empty() {
        return Err(format!("{name} has no text to check against"));
    }
    let total_chars = full.chars().count();
    if total_chars <= MAX_SOURCE_CHARS {
        return Ok(SourceText { name, text: full, trimmed: false, total_chars });
    }
    let mut cut: String = full.chars().take(MAX_SOURCE_CHARS).collect();
    // Back to the last paragraph end, when there is one in the second half.
    if let Some(i) = cut.rfind('\n').filter(|i| *i > cut.len() / 2) {
        cut.truncate(i);
    }
    Ok(SourceText { name, text: cut, trimmed: true, total_chars })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders(name: &str) -> (PathBuf, Vec<String>) {
        let base = std::env::temp_dir().join(format!("wc-source-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in ["allowed/sub", "allowed2", "outside"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        std::fs::write(base.join("allowed/facts.txt"), "The tower is 330 metres tall.\nIt opened in 1889.").unwrap();
        std::fs::write(base.join("allowed/sub/notes.md"), "# Notes\n\nParis is in France.").unwrap();
        std::fs::write(base.join("allowed/.hidden.txt"), "no").unwrap();
        std::fs::write(base.join("allowed/image.png"), "no").unwrap();
        std::fs::write(base.join("outside/secret.txt"), "secret").unwrap();
        let list = vec![base.join("allowed").to_string_lossy().into_owned()];
        (base, list)
    }

    fn s(p: PathBuf) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn a_file_inside_an_allowed_folder_is_read_as_text() {
        let (base, f) = folders("ok");
        let t = read(&s(base.join("allowed/facts.txt")), &f).unwrap();
        assert_eq!(t.name, "facts.txt");
        assert!(t.text.contains("330 metres") && !t.trimmed);
        let md = read(&s(base.join("allowed/sub/notes.md")), &f).unwrap();
        assert!(md.text.contains("Paris is in France"), "{md:?}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn nothing_outside_the_allowed_folders_is_read() {
        let (base, f) = folders("outside");
        for p in [
            s(base.join("outside/secret.txt")),
            s(base.join("allowed2")),
            format!("{}/../outside/secret.txt", s(base.join("allowed"))),
            "/etc/hosts".to_string(),
            "relative.txt".to_string(),
        ] {
            let e = read(&p, &f).unwrap_err();
            assert!(e.contains("outside the read root") || e.contains("can't read") || e.contains("cannot resolve"), "{p}: {e}");
        }
        // No folders at all: nothing is read.
        assert!(read(&s(base.join("allowed/facts.txt")), &[]).unwrap_err().contains("no folders are allowed"));
        // A sibling that shares the folder's name as a prefix is outside.
        std::fs::create_dir_all(base.join("allowed-more")).unwrap();
        std::fs::write(base.join("allowed-more/x.txt"), "x").unwrap();
        assert!(read(&s(base.join("allowed-more/x.txt")), &f).unwrap_err().contains("outside the read root"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leads_out_is_refused() {
        let (base, f) = folders("links");
        std::os::unix::fs::symlink(base.join("outside/secret.txt"), base.join("allowed/leak.txt")).unwrap();
        let e = read(&s(base.join("allowed/leak.txt")), &f).unwrap_err();
        assert!(e.contains("outside the read root"), "{e}");
        std::os::unix::fs::symlink(base.join("outside"), base.join("allowed/away")).unwrap();
        let e = read(&s(base.join("allowed/away/secret.txt")), &f).unwrap_err();
        assert!(e.contains("outside the read root"), "{e}");
        // A link to a file inside the folder is fine; a dangling one is refused.
        std::os::unix::fs::symlink(base.join("allowed/facts.txt"), base.join("allowed/alias.txt")).unwrap();
        assert!(read(&s(base.join("allowed/alias.txt")), &f).is_ok());
        std::os::unix::fs::symlink(base.join("allowed/none.txt"), base.join("allowed/dangling.txt")).unwrap();
        assert!(read(&s(base.join("allowed/dangling.txt")), &f).is_err());
        // The listing never follows links.
        let listed: Vec<String> = list(&f).iter().map(|p| clean_name(p)).collect();
        assert!(!listed.contains(&"leak.txt".to_string()) && !listed.contains(&"alias.txt".to_string()), "{listed:?}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn folders_unknown_types_and_big_files_are_refused() {
        let (base, f) = folders("kinds");
        let e = read(&s(base.join("allowed/sub")), &f).unwrap_err();
        assert!(e.contains("can't read"), "a folder has no type: {e}");
        std::fs::create_dir_all(base.join("allowed/folder.txt")).unwrap();
        let e = read(&s(base.join("allowed/folder.txt")), &f).unwrap_err();
        assert!(e.contains("not a file"), "{e}");
        let e = read(&s(base.join("allowed/image.png")), &f).unwrap_err();
        assert!(e.contains("can't read .png files"), "{e}");
        let e = read(&s(base.join("allowed/missing.txt")), &f).unwrap_err();
        assert!(!e.is_empty());
        // Over the size cap: refused before anything is read (a sparse file costs no disk).
        let big = base.join("allowed/big.txt");
        let file = std::fs::File::create(&big).unwrap();
        file.set_len(MAX_SOURCE_BYTES + 1).unwrap();
        let e = read(&s(big), &f).unwrap_err();
        assert!(e.contains("larger than 20 MB"), "{e}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn long_text_is_cut_at_a_paragraph_end() {
        let (base, f) = folders("long");
        let para = "Sentence about a fact. ".repeat(50);
        let text = (0..100).map(|_| para.clone()).collect::<Vec<_>>().join("\n");
        std::fs::write(base.join("allowed/long.txt"), &text).unwrap();
        let t = read(&s(base.join("allowed/long.txt")), &f).unwrap();
        assert!(t.trimmed && t.text.chars().count() <= MAX_SOURCE_CHARS && t.total_chars > MAX_SOURCE_CHARS);
        assert!(t.text.ends_with("fact. "), "cut at a paragraph end");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn damaged_files_are_an_error_not_a_crash() {
        let (base, f) = folders("damaged");
        for (name, bytes) in [("a.docx", &b"PK\x03\x04junk"[..]), ("b.odt", b"not a zip"), ("c.rtf", b"{\\rtf1 {{{"), ("d.txt", b"\xff\xfe\x00")] {
            std::fs::write(base.join("allowed").join(name), bytes).unwrap();
            let _ = read(&s(base.join("allowed").join(name)), &f);
        }
        std::fs::write(base.join("allowed/empty.txt"), "  \n").unwrap();
        assert!(read(&s(base.join("allowed/empty.txt")), &f).unwrap_err().contains("no text"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn the_listing_skips_hidden_files_and_other_types() {
        let (base, f) = folders("list");
        let names: Vec<String> = list(&f).iter().map(|p| clean_name(p)).collect();
        assert_eq!(names, ["facts.txt", "notes.md"]);
        assert!(list(&[]).is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn hostile_paths_never_panic() {
        let (base, f) = folders("hostile");
        for p in ["", "\0", "/", "..", "////", "a\u{0}b.txt", &"x".repeat(5000), "\u{202e}txt.exe", "~", "$HOME/.ssh/id_rsa.txt"] {
            let _ = read(p, &f);
            let _ = read(p, &[]);
            let _ = read(p, &[p.to_string(), "".into(), "\0".into()]);
        }
        let _ = std::fs::remove_dir_all(base);
    }
}
