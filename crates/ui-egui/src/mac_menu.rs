//! Margin's macOS menu bar, as data: the desktop app builds native menus from [`MENUS`] and calls
//! [`perform`] when one is chosen. Kept here (no AppKit) so it is testable and reachable over the
//! control channel (`ui.menu`).
//!
//! A native menu item with a key equivalent takes that keystroke before the window sees it. So an
//! item whose shortcut the app already handles replays the keystroke ([`Act::Keys`]): Bold in the
//! page, Copy in a text field, Undo anywhere behave exactly as typed.

use serde_json::{Value, json};

use crate::WordApp;
use crate::mac_ui::{self, set_pageless};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// Run a command (dialogs open when it needs input, as from a toolbar button).
    Cmd(&'static str),
    /// Run a command with JSON params.
    CmdWith(&'static str, &'static str),
    /// Replay a shortcut the app handles itself (`Mod+B`, `Mod+Shift+Z`…).
    Keys(&'static str),
    Copy,
    Cut,
    Paste,
    /// File › Export as PDF: ask where (the engine command needs a path), then write it.
    ExportPdf,
    Margin(MarginAct),
    /// Standard AppKit items (About, Hide, Quit, Minimize…): the native layer makes them.
    Std(Std),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarginAct {
    /// File › New: the template chooser (Pages-style), not a blank document straight away.
    Home,
    Outline,
    Inspector,
    Review,
    Pageless,
    Classic,
    /// Help › Search Commands…: focus the toolbar's command field (the old dialog in Classic).
    CommandField,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Std {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    Minimize,
    Zoom,
    Fullscreen,
    BringAllToFront,
}

#[derive(Clone, Copy, Debug)]
pub enum Entry {
    /// Label, action, macOS accelerator (muda syntax, e.g. `CmdOrCtrl+Shift+Z`).
    Item(&'static str, Act, Option<&'static str>),
    Sep,
    Sub(&'static str, &'static [Entry]),
}

use Act::*;
use Entry::{Item, Sep, Sub};

const fn it(label: &'static str, act: Act) -> Entry {
    Item(label, act, None)
}
const fn key(label: &'static str, act: Act, accel: &'static str) -> Entry {
    Item(label, act, Some(accel))
}

const APP: &[Entry] = &[
    it("About Margin", Std(self::Std::About)),
    Sep,
    key("Settings…", Cmd("file.options"), "CmdOrCtrl+Comma"),
    Sep,
    it("Services", Std(self::Std::Services)),
    Sep,
    it("Hide Margin", Std(self::Std::Hide)),
    it("Hide Others", Std(self::Std::HideOthers)),
    it("Show All", Std(self::Std::ShowAll)),
    Sep,
    it("Quit Margin", Std(self::Std::Quit)),
];

const FILE: &[Entry] = &[
    key("New…", Margin(MarginAct::Home), "CmdOrCtrl+N"),
    key("Open…", Keys("Mod+O"), "CmdOrCtrl+O"),
    Sep,
    key("Close", Keys("Mod+W"), "CmdOrCtrl+W"),
    key("Save", Keys("Mod+S"), "CmdOrCtrl+S"),
    key("Save As…", Cmd("file.saveAs"), "CmdOrCtrl+Shift+S"),
    Sep,
    it("Export as PDF…", ExportPdf),
    Sep,
    key("Print…", Keys("Mod+P"), "CmdOrCtrl+P"),
];

const EDIT: &[Entry] = &[
    key("Undo", Keys("Mod+Z"), "CmdOrCtrl+Z"),
    key("Redo", Keys("Mod+Shift+Z"), "CmdOrCtrl+Shift+Z"),
    Sep,
    key("Cut", Cut, "CmdOrCtrl+X"),
    key("Copy", Copy, "CmdOrCtrl+C"),
    key("Paste", Paste, "CmdOrCtrl+V"),
    key("Paste and Match Style", Cmd("edit.pasteText"), "CmdOrCtrl+Alt+Shift+V"),
    key("Select All", Keys("Mod+A"), "CmdOrCtrl+A"),
    Sep,
    key("Find…", Keys("Mod+F"), "CmdOrCtrl+F"),
    key("Find and Replace…", Cmd("edit.replace"), "CmdOrCtrl+Alt+F"),
    Sep,
    it("Spelling and Grammar", Cmd("review.spelling")),
];

const SHAPES: &[Entry] = &[
    it("Rectangle", CmdWith("insert.shape", r#"{"kind":"rectangle"}"#)),
    it("Rounded Rectangle", CmdWith("insert.shape", r#"{"kind":"roundedRectangle"}"#)),
    it("Oval", CmdWith("insert.shape", r#"{"kind":"ellipse"}"#)),
    it("Line", CmdWith("insert.shape", r#"{"kind":"line"}"#)),
    it("Arrow", CmdWith("insert.shape", r#"{"kind":"arrow"}"#)),
];

const INSERT: &[Entry] = &[
    it("Table…", Cmd("insert.table")),
    it("Image…", Cmd("insert.picture")),
    Sub("Shape", SHAPES),
    key("Link…", Keys("Mod+K"), "CmdOrCtrl+K"),
    Sep,
    key("Page Break", Keys("Mod+Enter"), "CmdOrCtrl+Enter"),
    key("Footnote", Keys("Mod+Alt+F"), "CmdOrCtrl+Alt+Shift+F"),
    it("Table of Contents", Cmd("references.toc")),
    Sep,
    it("Comment", Cmd("review.newComment")),
];

const STYLES: &[Entry] = &[
    it("Normal", CmdWith("para.style", r#"{"style":"Normal"}"#)),
    it("Title", CmdWith("para.style", r#"{"style":"Title"}"#)),
    it("Subtitle", CmdWith("para.style", r#"{"style":"Subtitle"}"#)),
    it("Heading 1", CmdWith("para.style", r#"{"style":"Heading1"}"#)),
    it("Heading 2", CmdWith("para.style", r#"{"style":"Heading2"}"#)),
    it("Heading 3", CmdWith("para.style", r#"{"style":"Heading3"}"#)),
];

const FORMAT: &[Entry] = &[
    key("Bold", Keys("Mod+B"), "CmdOrCtrl+B"),
    key("Italic", Keys("Mod+I"), "CmdOrCtrl+I"),
    key("Underline", Keys("Mod+U"), "CmdOrCtrl+U"),
    it("Strikethrough", Cmd("format.strikethrough")),
    Sep,
    key("Align Left", Keys("Mod+L"), "CmdOrCtrl+L"),
    key("Center", Keys("Mod+E"), "CmdOrCtrl+E"),
    key("Align Right", Keys("Mod+R"), "CmdOrCtrl+R"),
    key("Justify", Keys("Mod+J"), "CmdOrCtrl+J"),
    Sep,
    it("Bullets", Cmd("para.bullets")),
    it("Numbering", Cmd("para.numbering")),
    Sub("Paragraph Style", STYLES),
    Sep,
    key("Font…", Keys("Mod+D"), "CmdOrCtrl+D"),
    it("Paragraph…", Cmd("para.dialog")),
];

const REVIEW: &[Entry] = &[
    key("Track Changes", Keys("Mod+Shift+E"), "CmdOrCtrl+Shift+E"),
    it("Accept Change", Cmd("review.accept")),
    it("Reject Change", Cmd("review.reject")),
    it("Next Change", Cmd("review.nextChange")),
    Sep,
    key("New Comment", Keys("Mod+Alt+M"), "CmdOrCtrl+Alt+M"),
    Sep,
    it("Word Count", Cmd("review.wordCount")),
];

const VIEW: &[Entry] = &[
    key("Show Outline", Margin(MarginAct::Outline), "Control+Super+O"),
    key("Show Format Inspector", Margin(MarginAct::Inspector), "CmdOrCtrl+Alt+I"),
    key("Show Review", Margin(MarginAct::Review), "Control+Super+V"),
    it("Show Ruler", Cmd("view.ruler")),
    key("Show Invisibles", Keys("Mod+8"), "CmdOrCtrl+8"),
    it("Show Authorship", Cmd("view.authorship")),
    Sep,
    it("Pageless", Margin(MarginAct::Pageless)),
    it("Dark Page", Cmd("view.darkMode")),
    it("Focus", Cmd("view.focus")),
    Sep,
    key("Zoom In", Cmd("view.zoomIn"), "CmdOrCtrl+Equal"),
    key("Zoom Out", Cmd("view.zoomOut"), "CmdOrCtrl+Minus"),
    key("Actual Size", Cmd("view.zoom100"), "CmdOrCtrl+0"),
    it("Fit Width", Cmd("view.pageWidth")),
    Sep,
    key("Classic Ribbon", Margin(MarginAct::Classic), "Control+Super+R"),
    it("Enter Full Screen", Std(self::Std::Fullscreen)),
];

const WINDOW: &[Entry] =
    &[it("Minimize", Std(self::Std::Minimize)), it("Zoom", Std(self::Std::Zoom)), Sep, it("Bring All to Front", Std(self::Std::BringAllToFront))];

const HELP: &[Entry] = &[key("Search Commands…", Margin(MarginAct::CommandField), "CmdOrCtrl+Y")];

/// The menu bar, left to right. The first menu is the application menu.
pub const MENUS: &[(&str, &[Entry])] = &[
    ("Margin", APP),
    ("File", FILE),
    ("Edit", EDIT),
    ("Insert", INSERT),
    ("Format", FORMAT),
    ("Review", REVIEW),
    ("View", VIEW),
    ("Window", WINDOW),
    ("Help", HELP),
];

/// A toggle item's current state; `None` for plain items.
pub fn checked(app: &WordApp, act: &Act) -> Option<bool> {
    let v = &app.session.view;
    Some(match act {
        Margin(MarginAct::Outline) => v.nav_pane,
        Margin(MarginAct::Inspector) => app.ui.margin.inspector && mac_ui::active(app),
        Margin(MarginAct::Review) => v.comments_pane,
        Margin(MarginAct::Pageless) => mac_ui::pageless(app),
        Margin(MarginAct::Classic) => app.ui.margin.classic_ribbon,
        Keys("Mod+Shift+E") => app.session.doc.settings.track_changes,
        Keys("Mod+8") => v.marks,
        Cmd("view.ruler") => v.ruler,
        Cmd("view.darkMode") => v.dark_mode,
        Cmd("view.authorship") => v.authorship,
        Cmd("view.focus") => v.focus_mode,
        _ => return None,
    })
}

/// Do what a menu item stands for.
pub fn perform(app: &mut WordApp, act: &Act) {
    match *act {
        Cmd(id) => crate::widgets::run_from_ribbon(app, id, json!({})),
        CmdWith(id, params) => crate::widgets::run_from_ribbon(app, id, serde_json::from_str(params).unwrap_or(Value::Null)),
        Keys(combo) => {
            if let Some((key, modifiers)) = parse_combo(combo) {
                for pressed in [true, false] {
                    app.synthetic.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
                }
            }
        }
        ExportPdf => app.export_dialog("pdf"),
        Copy => app.synthetic.push(egui::Event::Copy),
        Cut => app.synthetic.push(egui::Event::Cut),
        // As the keyboard does: the clipboard's text, or with none, the bare key (pictures, #45).
        Paste => match app.services.clipboard_text.as_ref().and_then(|f| f()) {
            Some(t) if !t.is_empty() => app.synthetic.push(egui::Event::Paste(t)),
            _ => perform(app, &Keys("Mod+V")),
        },
        Margin(MarginAct::Home) => {
            app.ui.backstage = true;
            app.ui.backstage_page = "new".into();
        }
        Margin(MarginAct::Outline) => mac_ui::toggle_panel(app, mac_ui::Panel::Outline),
        Margin(MarginAct::Inspector) => mac_ui::toggle_panel(app, mac_ui::Panel::Format),
        Margin(MarginAct::Review) => mac_ui::toggle_panel(app, mac_ui::Panel::Review),
        Margin(MarginAct::Pageless) => {
            let on = !mac_ui::pageless(app);
            set_pageless(app, on);
        }
        Margin(MarginAct::Classic) => app.ui.margin.classic_ribbon = !app.ui.margin.classic_ribbon,
        Margin(MarginAct::CommandField) => crate::command_field::open(app, ""),
        // The native layer handles these itself.
        Std(_) => {}
    }
}

/// Every action in the menu bar, by its label path ("View/Pageless"), for the control channel.
pub fn find(path: &str) -> Option<Act> {
    fn walk(prefix: &str, entries: &[Entry], path: &str) -> Option<Act> {
        entries.iter().find_map(|e| match e {
            Item(l, a, _) if format!("{prefix}/{l}") == path => Some(*a),
            Sub(l, sub) => walk(&format!("{prefix}/{l}"), sub, path),
            _ => None,
        })
    }
    MENUS.iter().find_map(|(m, entries)| walk(m, entries, path))
}

/// `Mod+Shift+Z` → the key and macOS modifiers (Mod = ⌘).
fn parse_combo(combo: &str) -> Option<(egui::Key, egui::Modifiers)> {
    let mut m = egui::Modifiers::NONE;
    let mut key = None;
    for part in combo.split('+') {
        match part {
            "Mod" => {
                m.mac_cmd = true;
                m.command = true;
            }
            "Shift" => m.shift = true,
            "Alt" => m.alt = true,
            "Ctrl" => m.ctrl = true,
            k => key = egui::Key::from_name(k),
        }
    }
    key.map(|k| (k, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos_parse() {
        let (k, m) = parse_combo("Mod+Shift+Z").unwrap();
        assert_eq!(k, egui::Key::Z);
        assert!(m.mac_cmd && m.command && m.shift && !m.alt);
        assert_eq!(parse_combo("Mod+Enter").map(|(k, _)| k), Some(egui::Key::Enter));
        assert_eq!(parse_combo("Mod+8").map(|(k, _)| k), Some(egui::Key::Num8));
    }

    /// Every command the menus name exists, and every replayed shortcut parses.
    #[test]
    fn menu_commands_exist() {
        let session = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
        let ids: std::collections::HashSet<&str> = session.registry.all().iter().map(|c| c.id).collect();
        fn walk(entries: &[Entry], ids: &std::collections::HashSet<&str>) {
            for e in entries {
                match e {
                    Item(l, Cmd(id), _) => assert!(ids.contains(id), "{l}: unknown command {id}"),
                    Item(l, CmdWith(id, p), _) => {
                        assert!(id.starts_with("ui.") || ids.contains(id), "{l}: unknown command {id}");
                        assert!(serde_json::from_str::<Value>(p).is_ok(), "{l}: bad params");
                    }
                    Item(l, Keys(c), _) => assert!(parse_combo(c).is_some(), "{l}: {c}"),
                    Sub(_, s) => walk(s, ids),
                    _ => {}
                }
            }
        }
        for (_, m) in MENUS {
            walk(m, &ids);
        }
    }

    /// No menu item fails for want of input (#export-pdf, 2026-10-12): each command either runs
    /// with no params or opens the dialog that asks for them.
    #[test]
    fn every_menu_command_runs_or_asks() {
        fn walk(entries: &[Entry], out: &mut Vec<(&'static str, Act)>) {
            for e in entries {
                match e {
                    Item(l, a @ (Cmd(_) | CmdWith(..)), _) => out.push((l, *a)),
                    Sub(_, s) => walk(s, out),
                    _ => {}
                }
            }
        }
        let mut items = Vec::new();
        for (_, m) in MENUS {
            walk(m, &mut items);
        }
        for (label, act) in items {
            let mut a = WordApp::new(crate::Session::new(wordcraft_engine::sample::sample_document()), crate::Services::default());
            let (id, params) = match act {
                CmdWith(id, p) => (id, serde_json::from_str(p).unwrap()),
                Cmd(id) => (id, json!({})),
                _ => continue,
            };
            if let Err(e) = a.run(id, params) {
                assert!(!e.contains("bad parameters"), "{label} ({id}) needs input the menu doesn't give: {e}");
            }
        }
    }

    /// The menu's Bold replays ⌘B: the selection turns bold, as from the keyboard.
    #[test]
    fn bold_from_the_menu_acts_like_the_keyboard() {
        let mut a = WordApp::new(crate::Session::new(wordcraft_engine::sample::sample_document()), crate::Services::default());
        let _ = a.run("select.all", json!({}));
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        for _ in 0..6 {
            h.step();
        }
        h.state_mut().canvas.focused = true;
        let act = find("Format/Bold").unwrap();
        perform(h.state_mut(), &act);
        // As the desktop app's raw-input hook does: hand the replayed keystroke to egui.
        for _ in 0..3 {
            let mut raw = std::mem::take(h.input_mut());
            h.state_mut().raw_input_hook(&mut raw);
            *h.input_mut() = raw;
            h.step();
        }
        let st = h.state_mut().session.run("format.state", &json!({})).unwrap();
        assert_eq!(st.get("bold").and_then(Value::as_bool), Some(true), "{st}");
        assert_eq!(checked(h.state(), &find("View/Pageless").unwrap()), Some(false));
    }
}
