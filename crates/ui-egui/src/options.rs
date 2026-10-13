//! File › Options (#485): a list of panes on the left (General, Display, Proofing, Save,
//! Language, Accessibility, Advanced, Agents), the chosen pane's settings on the right. Only
//! settings WordCraft carries out are shown. Each pane is one function below, so a new setting
//! goes into its pane's function. Settings agents can change too go through `file.options`; the
//! rest live in [`crate::UiState`] (saved in `ui.json`).

use std::cell::Cell;

use egui::{RichText, Ui, vec2};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use wordcraft_engine::cmd::options::{MAX_WORD_CHARS, PANES};
use wordcraft_geom::Unit;

use crate::WordApp;
use crate::theme::{Tokens, semibold};

thread_local! {
    // The unit dialogs and the ribbon show lengths in: File › Options › Advanced, copied from the
    // session each frame so dialog code without the app can read it.
    static UNIT: Cell<Unit> = const { Cell::new(Unit::Inches) };
    // The region's decimal separator (#838).
    static DECIMAL: Cell<char> = const { Cell::new('.') };
}

/// The unit lengths are shown in (File › Options › Advanced › Measurement units, else the
/// region's).
pub fn unit() -> Unit {
    UNIT.get()
}

/// The decimal separator lengths are written with (the region's: `,` in Poland).
pub fn decimal() -> char {
    DECIMAL.get()
}

/// `pt` in [`unit`], written the region's way (`2,54 cm`).
pub fn length(pt: f32) -> String {
    unit().format_with(pt, decimal())
}

/// Copy the session's settings the UI reads without the app.
pub(crate) fn sync(app: &WordApp) {
    crate::assistant_link::sync_names(&app.ui.assistant);
    UNIT.set(app.session.units());
    DECIMAL.set(app.session.region().decimal());
}

/// A length field: the unit's suffix, and numbers written and read with the region's decimal
/// separator (`2,54 cm`). Typing either separator works, and so does another unit (`1 in` in a
/// centimetre field).
pub trait MeasureExt {
    fn measure(self, unit: Unit) -> Self;
}

impl MeasureExt for egui::DragValue<'_> {
    fn measure(self, unit: Unit) -> Self {
        let sep = decimal();
        self.suffix(unit.suffix())
            .custom_formatter(move |n, decimals| {
                let s = egui::emath::format_with_decimals_in_range(n, decimals);
                if sep == '.' { s } else { s.replace('.', &sep.to_string()) }
            })
            .custom_parser(move |text| {
                let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                wordcraft_geom::parse_measure(&text, unit).map(|pt| f64::from(pt / unit.pt_per_unit()))
            })
    }
}

/// Regions offered in File › Options › Advanced › Region (any locale tag works through
/// `file.options`): Letter and A4 countries, decimal point and decimal comma.
const REGIONS: [&str; 16] =
    ["en-US", "en-CA", "en-GB", "en-AU", "en-IN", "es-MX", "es-ES", "pt-BR", "fr-FR", "de-DE", "it-IT", "pl-PL", "ru-RU", "ja-JP", "zh-CN", "ko-KR"];

/// The units offered, with their names.
pub const UNITS: [(Unit, &str); 5] = [
    (Unit::Inches, "Inches"),
    (Unit::Centimeters, "Centimeters"),
    (Unit::Millimeters, "Millimeters"),
    (Unit::Points, "Points"),
    (Unit::Picas, "Picas"),
];

/// The pane names, in [`PANES`] order.
fn pane_label(id: &str) -> &'static str {
    match id {
        "display" => "Display",
        "proofing" => "Proofing",
        "save" => "Save",
        "language" => "Language",
        "accessibility" => "Accessibility",
        "advanced" => "Advanced",
        "agents" if crate::skin::on() => "Assistant",
        "agents" => "Agents",
        _ => "General",
    }
}

/// The Options page.
pub fn page(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(tl!("Options")).font(semibold(26.0)));
    ui.add_space(16.0);
    let current = if PANES.contains(&app.ui.options_pane.as_str()) { app.ui.options_pane.clone() } else { "general".to_string() };
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(170.0);
            for id in PANES {
                let label = tl!(pane_label(id));
                let active = current == id;
                let (r, resp) = ui.allocate_exact_size(vec2(170.0, 30.0), egui::Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label));
                if active {
                    ui.painter().rect_filled(r, 4.0, t.checked);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 4.0, t.hover);
                }
                let font = if active { semibold(14.0) } else { crate::theme::regular(14.0) };
                ui.painter().text(egui::pos2(r.min.x + 12.0, r.center().y), egui::Align2::LEFT_CENTER, label, font, t.text);
                if resp.clicked() {
                    app.ui.options_pane = id.to_string();
                }
            }
        });
        ui.add_space(12.0);
        let (line, _) = ui.allocate_exact_size(vec2(1.0, 420.0), egui::Sense::hover());
        ui.painter().rect_filled(line, 0.0, t.border);
        ui.add_space(20.0);
        ui.vertical(|ui| {
            ui.set_max_width(640.0);
            match current.as_str() {
                "display" => display(app, ui),
                "proofing" => proofing(app, ui),
                "save" => save(app, ui),
                "language" => language(app, ui),
                "accessibility" => accessibility(app, ui),
                "advanced" => advanced(app, ui),
                "agents" if crate::skin::on() => assistant(app, ui),
                "agents" => agents(ui),
                _ => general(app, ui),
            }
        });
    });
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(tl!(title)).font(semibold(15.0)));
    ui.add_space(2.0);
}

fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(tl!(text)).small().weak());
}

/// Change settings through `file.options` (the command agents use).
fn set(app: &mut WordApp, params: Value) {
    if let Err(e) = app.run("file.options", params) {
        app.status(e);
    }
}

/// A checkbox for a `file.options` on/off setting.
fn flag(app: &mut WordApp, ui: &mut Ui, label: &str, key: &str, value: bool) {
    let mut v = value;
    if ui.checkbox(&mut v, tl!(label)).changed() {
        set(app, json!({ key: v }));
    }
}

fn general(app: &mut WordApp, ui: &mut Ui) {
    section(ui, "Interface");
    theme_picker(app, ui);
    interface_size_picker(app, ui);
    ui.checkbox(&mut app.ui.mini_toolbar, tl!("Show the mini toolbar when text is selected"));
    let mut dark_page = app.session.view.dark_mode;
    if ui
        .checkbox(&mut dark_page, tl!("Dark page (white text on black)"))
        .on_hover_text(tl!("Show documents with their colours inverted, like View › Switch Modes. Saving, printing and PDFs are unchanged."))
        .changed()
    {
        let _ = app.run("view.darkMode", json!({"value": dark_page}));
    }
    ui.checkbox(&mut app.ui.show_discord, tl!("Show the community button in the title bar"));
    default_app_button(app, ui);
    ui.add_space(8.0);
    section(ui, "Start-up");
    let mut start = app.ui.start_screen;
    if ui
        .checkbox(&mut start, tl!("Show the Start screen when WordCraft starts"))
        .on_hover_text(tl!(
            "On: WordCraft opens on File › Home, with a blank document, the templates and your recent documents. Off: it opens a blank document."
        ))
        .changed()
    {
        let _ = app.run("ui.startScreen", json!({"value": start}));
    }
    ui.add_space(8.0);
    section(ui, "Personalize");
    egui::Grid::new("options_user").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
        ui.label(tl!("User name:"));
        let mut n = app.session.author.clone();
        if ui.add(egui::TextEdit::singleline(&mut n).desired_width(260.0)).changed() {
            app.session.author = n;
        }
        ui.end_row();
        ui.label(tl!("Initials:"));
        let mut i = app.session.initials.clone();
        let hint = app.session.user_initials();
        if ui
            .add(
                egui::TextEdit::singleline(&mut i).desired_width(80.0).char_limit(wordcraft_engine::cmd::options::MAX_INITIALS_CHARS).hint_text(hint),
            )
            .changed()
        {
            set(app, json!({"initials": i}));
        }
        ui.end_row();
    });
    note(ui, "New comments and tracked changes are recorded under this name and these initials.");
}

fn display(app: &mut WordApp, ui: &mut Ui) {
    section(ui, "Formatting marks");
    note(ui, "These marks show even when Show/Hide ¶ is off.");
    let m = app.session.prefs.marks;
    for (label, key, on) in [
        ("Tab characters", "tabs", m.tabs),
        ("Spaces", "spaces", m.spaces),
        ("Paragraph marks", "paragraphs", m.paragraphs),
        ("Hidden text", "hidden", m.hidden),
    ] {
        let mut v = on;
        if ui.checkbox(&mut v, tl!(label)).changed() {
            set(app, json!({"marks": { key: v }}));
        }
    }
    let mut marks = app.session.view.marks;
    if ui.checkbox(&mut marks, tl!("Show all formatting marks")).changed() {
        let _ = app.run("view.marks", json!({"value": marks}));
    }
    ui.add_space(8.0);
    section(ui, "Page display");
    let mut ruler = app.session.view.ruler;
    if ui.checkbox(&mut ruler, tl!("Show rulers")).changed() {
        let _ = app.run("view.ruler", json!({"value": ruler}));
    }
    numeral_picker(app, ui);
    ui.add_space(8.0);
    section(ui, "Printing");
    ui.checkbox(&mut app.ui.print_backgrounds, tl!("Print the page color"));
    ui.checkbox(&mut app.ui.update_fields_before_print, tl!("Update fields before printing"));
}

fn proofing(app: &mut WordApp, ui: &mut Ui) {
    // #476 (AutoCorrect Options…) goes here, at the top.
    section(ui, "Spelling");
    let pr = app.session.prefs.clone();
    flag(app, ui, "Ignore words in capitals", "ignoreUppercase", pr.ignore_uppercase);
    flag(app, ui, "Ignore words with numbers", "ignoreNumbers", pr.ignore_numbers);
    flag(app, ui, "Ignore web and file addresses", "ignoreInternet", pr.ignore_internet);
    ui.add_space(8.0);
    section(ui, "As you type");
    let spelling = app.session.view.proofing;
    flag(app, ui, "Check spelling as you type", "checkSpelling", spelling);
    ui.add_enabled_ui(app.session.view.proofing, |ui| flag(app, ui, "Mark grammar errors as you type", "markGrammar", pr.mark_grammar));
    ui.add_space(8.0);
    section(ui, "Custom dictionary");
    custom_dictionary(app, ui);
}

/// The custom dictionary: the words Add to Dictionary kept, each with a Remove button, and a
/// field to add one.
fn custom_dictionary(app: &mut WordApp, ui: &mut Ui) {
    let words = wordcraft_engine::proof::user_dictionary();
    if words.is_empty() {
        note(ui, "No words yet. Add to Dictionary on a misspelled word puts it here.");
    }
    egui::ScrollArea::vertical().id_salt("options_dictionary").max_height(180.0).show(ui, |ui| {
        for w in &words {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(220.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(220.0);
                    ui.add(egui::Label::new(w.as_str()).truncate());
                });
                if ui.small_button(tl!("Remove")).clicked() {
                    set(app, json!({"removeWords": [w]}));
                }
            });
        }
    });
    let id = egui::Id::new("options_dictionary_new");
    let mut word = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut word).desired_width(220.0).char_limit(MAX_WORD_CHARS).hint_text(tl!("New word")));
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let w = word.trim().to_string();
        let ok = !w.is_empty() && !w.chars().any(char::is_whitespace);
        if (ui.add_enabled(ok, egui::Button::new(tl!("Add"))).clicked() || (enter && ok)) && app.run("file.options", json!({"addWords": [w]})).is_ok()
        {
            word.clear();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, word));
}

fn save(app: &mut WordApp, ui: &mut Ui) {
    section(ui, "Save documents");
    // The browser can't write to the user's files, so AutoSave can't be turned on there (#176).
    let browser = app.autosave_block() == Some(crate::AutoSaveBlock::Browser);
    let mut autosave = app.autosave && !browser;
    let r = ui.add_enabled(!browser, egui::Checkbox::new(&mut autosave, tl!("AutoSave documents you have saved in WordCraft")));
    if browser {
        r.on_disabled_hover_text(crate::AutoSaveBlock::Browser.reason());
    } else if r.changed() {
        app.autosave = autosave;
        app.session.autosave = autosave;
    }
    note(ui, "AutoSave writes your changes to the file a moment after you stop typing.");
}

fn language(app: &mut WordApp, ui: &mut Ui) {
    section(ui, "Interface language");
    language_picker(app, ui);
    // #414 (detect the document's language) goes here.
    ui.add_space(8.0);
    section(ui, "Proofing language");
    note(ui, "Spelling and grammar are checked in English (United States).");
}

fn accessibility(app: &mut WordApp, ui: &mut Ui) {
    use wordcraft_engine::speech::{MAX_RATE, MIN_RATE};
    section(ui, "Visual effects");
    animations_picker(app, ui);
    ui.add_space(8.0);
    section(ui, "Read Aloud");
    let mut rate = app.session.read_aloud.rate();
    ui.horizontal(|ui| {
        ui.label(tl!("Speed"));
        let r = ui.add(egui::Slider::new(&mut rate, MIN_RATE..=MAX_RATE).step_by(0.05).custom_formatter(|v, _| format!("{v:.2}×")));
        if r.changed() {
            let _ = app.run("readAloud.speed", json!({"value": rate}));
        }
    });
    let mut skip = app.session.read_aloud.skip_citations;
    if ui.checkbox(&mut skip, tl!("Skip citations & bibliography")).changed() {
        let _ = app.run("readAloud.skipCitations", json!({"value": skip}));
    }
}

fn advanced(app: &mut WordApp, ui: &mut Ui) {
    section(ui, "Editing");
    let pr = app.session.prefs.clone();
    flag(app, ui, "Typing replaces selected text", "typingReplacesSelection", pr.typing_replaces_selection);
    flag(app, ui, "Use the Insert key to switch overtype on and off", "insertKeyOvertype", pr.insert_key_overtype);
    flag(app, ui, "Overtype (typing replaces the characters after the caret)", "overtype", pr.overtype);
    ui.add_space(8.0);
    section(ui, "Display");
    egui::Grid::new("options_display").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
        ui.label(tl!("Recent documents to show:"));
        let mut n = app.ui.recent_shown();
        if ui.add(egui::DragValue::new(&mut n).range(0..=crate::MAX_RECENT).speed(0.2)).changed() {
            app.ui.recent_count = n;
        }
        ui.end_row();
        ui.label(tl!("Measurement units:"));
        // The user's choice, else the region's (#838): Automatic follows the region.
        let units = app.session.units();
        let auto = app.session.prefs.units.is_none();
        let name = UNITS.iter().find(|(u, _)| *u == units).map_or("Inches", |(_, n)| n);
        let shown = if auto { format!("{} ({})", tl!("Automatic"), tl!(name)) } else { tl!(name).to_string() };
        egui::ComboBox::from_id_salt("options_units").selected_text(shown).width(200.0).show_ui(ui, |ui| {
            if ui.selectable_label(auto, tl!("Automatic")).clicked() {
                set(app, json!({"units": "auto"}));
            }
            for (u, label) in UNITS {
                if ui.selectable_label(!auto && units == u, tl!(label)).clicked() {
                    set(app, json!({"units": u}));
                }
            }
        });
        ui.end_row();
        // The region new documents, units and the decimal separator follow; Automatic is the
        // system's (#838).
        ui.label(tl!("Region:"));
        let chosen = app.session.prefs.region.clone();
        let system = &app.session.system_region;
        let system_tag = if system.territory.is_empty() { system.language.clone() } else { format!("{}-{}", system.language, system.territory) };
        let automatic = crate::i18n::fmt(tl!("Automatic ({region})"), &[("region", system_tag.as_str())]);
        let shown = chosen.clone().unwrap_or_else(|| automatic.clone());
        egui::ComboBox::from_id_salt("options_region").selected_text(shown).width(200.0).show_ui(ui, |ui| {
            if ui.selectable_label(chosen.is_none(), automatic.as_str()).clicked() {
                set(app, json!({"region": "auto"}));
            }
            for tag in REGIONS {
                if ui.selectable_label(chosen.as_deref() == Some(tag), tag).clicked() {
                    set(app, json!({"region": tag}));
                }
            }
        });
        ui.end_row();
    });
    let mut keytips = app.ui.keytips_enabled;
    if ui.checkbox(&mut keytips, tl!("Show KeyTips (Alt or F10)")).changed() {
        let _ = app.run("ui.keytips", json!({"value": keytips}));
    }
    ui.add_space(8.0);
    section(ui, "Animation");
    let mut caret = app.ui.caret_animation;
    if ui
        .checkbox(&mut caret, tl!("Animate the caret (it glides and fades)"))
        .on_hover_text(tl!(
            "On: the caret glides to its new place when you type or use the arrow keys, and its blink fades. Off: it jumps and blinks on and off."
        ))
        .changed()
    {
        let _ = app.run("ui.caretAnimation", json!({"value": caret}));
    }
    ui.add_space(8.0);
    section(ui, "Customize");
    ui.horizontal(|ui| {
        ui.label(tl!("Keyboard shortcuts:"));
        if ui.button(tl!("Customize…")).clicked() {
            let _ = app.run("tools.customizeKeyboard", json!({}));
        }
    });
    ui.horizontal(|ui| {
        ui.label(tl!("Ribbon and Quick Access Toolbar:"));
        if ui.button(tl!("Customize…")).clicked() {
            let _ = app.run("tools.customizeRibbon", json!({}));
        }
    });
}

fn agents(ui: &mut Ui) {
    section(ui, "Agents");
    ui.label(tl!("Every command is available to scripts and AI agents: run `wordcraft-cli mcp` for an MCP server, or start the app with `--control <port>` for the JSON control channel."));
}

/// Options › Assistant (Margin, HANDOFF §9): the master switch (S1), provider and model (S2), the
/// API key (S3), the name assistants write under (S4), the rule that their edits are always
/// suggestions, comments and the folders they may read (S5), and whether assistants outside
/// Margin may connect (S6).
fn assistant(app: &mut WordApp, ui: &mut Ui) {
    use crate::mac_controls as mc;
    // The connected count changes on other threads: look again soon.
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    mc::section(ui, "Assistant", |ui| {
        if mc::switch(ui, "Let an assistant work in Margin", app.ui.assistant.enabled) {
            let on = !app.ui.assistant.enabled;
            let _ = app.run("ui.assistant", json!({"enabled": on}));
        }
        mc::hint(ui, "Off: nothing is sent anywhere, and the Assistant panel and the Ask rows are hidden. You bring the provider and its key.");
    });
    assistant_provider(app, ui);
    assistant_key(app, ui);
    mc::section(ui, "Author", |ui| {
        mc::labelled_row(ui, "Author name", |ui| {
            // The prefix is fixed: shown before the field, never part of it.
            ui.label(egui::RichText::new(wordcraft_engine::authorship::AI_PREFIX.trim_end()).font(mc::font()));
            let hint = crate::i18n::fmt(tl!("{name} (from the model)"), &[("name", &app.ui.assistant.derived_name())]);
            let before = app.ui.assistant.author_name.clone();
            let r = ui.add(egui::TextEdit::singleline(&mut app.ui.assistant.author_name).desired_width(220.0).hint_text(hint));
            if r.gained_focus() {
                app.ui.assistant.remember_name(&before);
            }
            if r.lost_focus() {
                let typed = crate::assistant::clean_name(&app.ui.assistant.author_name);
                app.ui.assistant.author_name = typed.trim_start_matches(wordcraft_engine::authorship::AI_PREFIX).trim().to_string();
            }
            if r.changed() || r.lost_focus() {
                crate::assistant_link::sync_names(&app.ui.assistant);
            }
        });
        let shown = crate::i18n::fmt(tl!("Shown as: {name}"), &[("name", &app.ui.assistant.name())]);
        ui.label(egui::RichText::new(shown).font(mc::font()).color(Tokens::get(ui.ctx()).text_dim));
        mc::hint(
            ui,
            "Optional. Written into the changes and comments an assistant makes, always after “AI · ”, so they are recognised anywhere. Its mark is always dotted.",
        );
    });
    mc::section(ui, "Permissions", |ui| {
        // Required, so shown on and not changeable.
        ui.add_enabled_ui(false, |ui| {
            mc::checkbox(ui, "Always suggest as tracked changes", true);
        });
        mc::hint(ui, "An assistant's edits never change your text until you accept them in Review.");
        ui.add_space(6.0);
        if mc::checkbox(ui, "Add and reply to comments", app.ui.assistant.comments) {
            let on = !app.ui.assistant.comments;
            let _ = app.run("ui.assistant", json!({"comments": on}));
        }
        mc::hint(ui, "Off: Margin's assistant adds no comments, and assistants outside Margin are refused new comments and replies.");
        ui.add_space(6.0);
        assistant_folders(app, ui);
    });
    mc::section(ui, "Outside Margin", |ui| {
        if mc::switch(ui, "Allow assistants outside Margin to connect", app.ui.assistant.allow_outside) {
            let on = !app.ui.assistant.allow_outside;
            let _ = app.run("ui.assistant", json!({"allowOutside": on}));
        }
        let n = crate::assistant::outside::connected();
        ui.horizontal(|ui| {
            let t = Tokens::get(ui.ctx());
            let line = crate::i18n::fmt(tl!("{n} connected now"), &[("n", &n.to_string())]);
            ui.label(RichText::new(format!("{line} ·")).color(t.text_dim));
            ui.add_enabled_ui(n > 0, |ui| {
                if mc::link(ui, "Disconnect").clicked() {
                    crate::assistant::outside::disconnect_all();
                }
            });
        });
        mc::hint(
            ui,
            "Claude Desktop, Claude Code and other MCP clients reach this window through `wordcraft-cli mcp --connect <port>`, once the app is started with `--control <port>`. Whatever they change arrives as a tracked change under the name above; they can't accept or reject changes, turn tracking off or change these settings.",
        );
        mc::hint(ui, "`wordcraft-cli mcp` without the app edits files directly. This setting doesn't cover it.");
    });
}

/// S5: "Read files in these folders (read-only)": the folder list with + and −. The same list
/// confines the files assistants outside Margin can read (`control::handle_outside`).
fn assistant_folders(app: &mut WordApp, ui: &mut Ui) {
    use crate::mac_controls as mc;
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(tl!("Read files in these folders (read-only)")).font(mc::font()).color(t.text));
    let mut remove: Option<String> = None;
    let folders = app.ui.assistant.folders.clone();
    egui::Frame::NONE
        .fill(t.input)
        .stroke(egui::Stroke::new(1.0, t.input_border))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if folders.is_empty() {
                ui.label(RichText::new(tl!("No folders: assistants can't read any file.")).font(mc::font()).color(t.text_dim));
            }
            let list_w = ui.available_width();
            for f in &folders {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (r, resp) = ui.allocate_exact_size(vec2(list_w - 32.0, mc::ROW_H), egui::Sense::hover());
                    // The folder's name, then where it is (dim, cut at the end).
                    let path = std::path::Path::new(f);
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| f.clone());
                    let parent = path.parent().map(|p| tidy_home(&p.to_string_lossy())).unwrap_or_default();
                    let clip = ui.painter().with_clip_rect(r);
                    let used = clip.text(egui::pos2(r.min.x, r.center().y), egui::Align2::LEFT_CENTER, &name, mc::font(), t.text).width();
                    clip.text(egui::pos2(r.min.x + used + 10.0, r.center().y), egui::Align2::LEFT_CENTER, parent, mc::font(), t.text_dim);
                    resp.on_hover_text(f.as_str());
                    if small_button(ui, "−", &crate::i18n::fmt(tl!("Remove {folder}"), &[("folder", f)])) {
                        remove = Some(f.clone());
                    }
                });
            }
        });
    if let Some(f) = remove {
        let _ = app.run("ui.assistant", json!({"removeFolder": f}));
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let full = app.ui.assistant.folders.len() >= crate::assistant::MAX_FOLDERS;
        ui.add_enabled_ui(!full, |ui| {
            if small_button(ui, "+", tl!("Add a folder…")) {
                let _ = app.run("ui.assistant", json!({"pickFolder": true}));
            }
        });
        ui.label(RichText::new(tl!("Add a folder…")).font(mc::font()).color(t.text_dim));
    });
    mc::hint(
        ui,
        "Margin's assistant can check facts against files in these folders, and assistants outside Margin can read files only from them. Nothing is ever written, and links that lead out of a folder are not followed. No folders: no file access.",
    );
}

/// `path` with the home folder written as `~`.
fn tidy_home(path: &str) -> String {
    match std::env::var("HOME").ok().filter(|h| h.len() > 1) {
        Some(h) => match path.strip_prefix(h.as_str()) {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
            _ => path.to_string(),
        },
        None => path.to_string(),
    }
}

/// A 24 × 24 square button with a glyph (+ or −); true when clicked.
fn small_button(ui: &mut Ui, glyph: &str, tip: &str) -> bool {
    use crate::mac_controls as mc;
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
    let fill = if resp.hovered() { t.hover } else { egui::Color32::TRANSPARENT };
    ui.painter().rect(r, 6.0, fill, egui::Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, glyph, mc::font(), t.text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tip));
    resp.on_hover_text(tip).clicked()
}

/// S2: provider radios, the model popup (filled live from the provider) and, for On this Mac and
/// Other, the server address.
fn assistant_provider(app: &mut WordApp, ui: &mut Ui) {
    use crate::mac_controls as mc;
    use wordcraft_assistant::ProviderId;
    mc::section(ui, "Provider", |ui| {
        let current = app.ui.assistant.provider_id();
        for p in ProviderId::ALL {
            if mc::radio(ui, p.label(), p == current) && p != current {
                let _ = app.run("ui.assistant", json!({"provider": p.id()}));
            }
        }
        let p = app.ui.assistant.provider_id();
        if p.preset().editable_url {
            mc::labelled_row(ui, "Address", |ui| {
                let id = egui::Id::new(("assistant_url", p.id()));
                let mut url: String =
                    ui.data(|d| d.get_temp(id)).unwrap_or_else(|| app.ui.assistant.base_urls.get(p.id()).cloned().unwrap_or_default());
                let hint = if p == ProviderId::Local { wordcraft_assistant::LOCAL_BASE_URL } else { "https://…/v1" };
                let r = ui.add(egui::TextEdit::singleline(&mut url).desired_width(300.0).hint_text(hint));
                if r.lost_focus() {
                    ui.data_mut(|d| d.remove::<String>(id));
                    if let Err(e) = app.run("ui.assistant", json!({"baseUrl": url})) {
                        app.assistant_link.last = Some(crate::assistant_link::LastTest { provider: p, result: Err(e) });
                    }
                } else if r.changed() || r.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, url));
                }
            });
            if p == ProviderId::Local {
                mc::hint(ui, "Ollama listens on http://localhost:11434/v1, LM Studio on http://localhost:1234/v1.");
            }
        }
        mc::labelled_row(ui, "Model", |ui| {
            let models = app.assistant_link.models.get(&p).cloned().unwrap_or_default();
            let chosen = app.ui.assistant.model(p).map(str::to_string);
            if models.is_empty() {
                // Not listed (yet, or the listing failed): the id can be typed.
                let mut m = chosen.clone().unwrap_or_default();
                let r = ui.add(egui::TextEdit::singleline(&mut m).desired_width(220.0).hint_text(tl!("Model id")));
                if r.changed() {
                    let _ = app.run("ui.assistant", json!({"model": m}));
                }
            } else {
                let shown = chosen
                    .as_deref()
                    .map(|id| models.iter().find(|x| x.id == id).map_or(id.to_string(), |x| x.name.clone()))
                    .unwrap_or_else(|| tl!("Choose a model").to_string());
                let names: Vec<String> = models.iter().map(|x| x.name.clone()).collect();
                if let Some(name) = mc::popup_button(ui, "assistant_model", 260.0, &shown, &names, names.len() > 12)
                    && let Some(m) = models.iter().find(|x| x.name == name)
                {
                    let _ = app.run("ui.assistant", json!({"model": m.id}));
                }
            }
            let can = app.ui.assistant.enabled && !app.assistant_link.busy();
            ui.add_enabled_ui(can, |ui| {
                if mc::link(ui, "Refresh").clicked() {
                    let _ = app.run("ui.assistant.models", json!({"refresh": true}));
                }
            });
        });
        mc::hint(ui, "The list comes from the provider once a key is set.");
    });
}

/// S3: the API key (only `••••` and its last four characters are shown), Change… / Remove, and
/// Test connection with its outcome.
fn assistant_key(app: &mut WordApp, ui: &mut Ui) {
    use crate::mac_controls as mc;
    let p = app.ui.assistant.provider_id();
    let needs = p.preset().needs_key;
    mc::section(ui, "API key", |ui| {
        let t = Tokens::get(ui.ctx());
        let hint = app.assistant_link.hint(p);
        if !app.assistant_link.keys_supported() {
            mc::hint(ui, "This system has no keychain Margin can use, so a key can't be stored here.");
        } else if let Some(h) = hint.as_ref().filter(|_| !app.assistant_link.changing_key) {
            mc::labelled_row(ui, "Key", |ui| {
                ui.label(RichText::new(h).font(mc::font()).color(t.text));
                ui.add_space(8.0);
                if mc::link(ui, "Change…").clicked() {
                    app.assistant_link.changing_key = true;
                    app.assistant_link.key_draft.clear();
                }
                if mc::link(ui, "Remove").clicked() {
                    let _ = app.run("ui.assistant", json!({"removeKey": true}));
                }
            });
        } else {
            mc::labelled_row(ui, "Key", |ui| {
                let field = egui::TextEdit::singleline(&mut app.assistant_link.key_draft).password(true).desired_width(260.0).hint_text(if needs {
                    tl!("Paste your API key")
                } else {
                    tl!("Not needed for most local servers")
                });
                let r = ui.add(field);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let has_draft = !app.assistant_link.key_draft.trim().is_empty();
                if (mc::button(ui, "Save", false, has_draft).clicked() || enter) && has_draft {
                    crate::assistant_link::save_draft_key(app);
                }
                if app.assistant_link.changing_key && mc::link(ui, "Cancel").clicked() {
                    app.assistant_link.changing_key = false;
                    app.assistant_link.key_draft.clear();
                }
            });
            if let Some(e) = app.assistant_link.key_error.clone() {
                ui.label(RichText::new(sentence(&e)).font(mc::font()).color(t.red));
            }
        }
        ui.horizontal(|ui| {
            let on = app.ui.assistant.enabled;
            let busy = app.assistant_link.busy();
            if mc::button(ui, "Test connection", false, on && !busy).clicked()
                && let Err(e) = app.run("ui.assistant.test", json!({}))
            {
                app.assistant_link.last = Some(crate::assistant_link::LastTest { provider: p, result: Err(e) });
            }
            ui.add_space(8.0);
            if busy {
                ui.add(egui::Spinner::new().size(14.0).color(t.text_dim));
                ui.label(RichText::new(tl!("Connecting…")).font(mc::font()).color(t.text_dim));
            } else if let Some(last) = app.assistant_link.last.as_ref().filter(|l| l.provider == p) {
                match &last.result {
                    Ok(n) => {
                        let line = crate::i18n::fmt(tl!("Connected · {n} models"), &[("n", &n.to_string())]);
                        ui.label(RichText::new(line).font(mc::font()).color(t.green));
                    }
                    Err(e) => {
                        ui.add(egui::Label::new(RichText::new(sentence(e)).font(mc::font()).color(t.red)).wrap());
                    }
                }
            } else if !on {
                ui.label(RichText::new(tl!("Turn the assistant on to connect.")).font(mc::font()).color(t.text_dim));
            }
        });
        let store = if cfg!(target_os = "macos") { "the macOS Keychain" } else { "the system keychain" };
        let line = crate::i18n::fmt(tl!("Keys are kept in {store}, one per provider, never in Margin's settings files."), &[("store", tl!(store))]);
        mc::hint(ui, &line);
    });
}

/// An error message as a sentence: capital first letter and a full stop.
fn sentence(e: &str) -> String {
    let mut c = e.chars();
    let mut s: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
    if !s.ends_with(['.', '!', '?']) {
        s.push('.');
    }
    s
}

/// Interface theme: Light, Dark, or follow the system's appearance (#115).
fn theme_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::theme::Appearance;
    ui.horizontal(|ui| {
        ui.label(tl!("Interface theme:"));
        egui::ComboBox::from_id_salt("interface_theme").selected_text(tl!(app.ui.theme.label())).width(220.0).show_ui(ui, |ui| {
            for a in Appearance::ALL {
                if ui.selectable_label(app.ui.theme == a, tl!(a.label())).clicked() {
                    let _ = app.run("ui.theme", json!({"value": a.code()}));
                }
            }
        });
    });
}

/// Interface size: 80–200 %, on top of the system's display scale (#475).
fn interface_size_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::theme::InterfaceSize;
    let current = app.ui.interface_size.percent();
    ui.horizontal(|ui| {
        ui.label(tl!("Interface size:"));
        egui::ComboBox::from_id_salt("interface_size").selected_text(format!("{current}%")).width(220.0).show_ui(ui, |ui| {
            for pct in InterfaceSize::PRESETS {
                if ui.selectable_label(current == pct, format!("{pct}%")).clicked() {
                    let _ = app.run("ui.interfaceSize", json!({"percent": pct}));
                }
            }
        });
    });
    ui.label(
        RichText::new(tl!(
            "Makes the ribbon, menus and dialogs larger or smaller, on top of your system's display scaling. Document zoom is separate."
        ))
        .small()
        .weak(),
    );
}

/// Animation effects: on, off, or follow the system's setting (the default). Under "Use system
/// setting" a line says what the system reports, so it is plain what the choice comes to here.
fn animations_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::theme::Animations;
    ui.horizontal(|ui| {
        ui.label(tl!("Animation effects:"));
        egui::ComboBox::from_id_salt("animation_effects").selected_text(tl!(app.ui.animations.label())).width(220.0).show_ui(ui, |ui| {
            for a in Animations::ALL {
                if ui.selectable_label(app.ui.animations == a, tl!(a.label())).clicked() {
                    let _ = app.run("ui.animations", json!({"value": a.code()}));
                }
            }
        });
    });
    note(ui, "Smooth scrolling and zooming, and menus and buttons that fade in. Off shows every change at once.");
    if app.ui.animations == Animations::System {
        let reported = match app.system_animations() {
            Some(true) => "Your system has animation effects turned on.",
            Some(false) => "Your system has animation effects turned off.",
            None => "Your system doesn't report an animation setting, so animation effects stay on.",
        };
        note(ui, reported);
    }
}

/// Interface language: follow the system (the default) or pick one (#8).
fn language_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::i18n::{AUTO, Lang};
    // The language names are in their own scripts: fonts for them load from the next frame.
    app.want_system_cjk = true;
    let system = crate::i18n::system_lang();
    let auto_label = crate::i18n::fmt(tl!("Automatic ({language})"), &[("language", system.name())]);
    let current = if app.ui.language == AUTO { auto_label.clone() } else { Lang::from_pref(&app.ui.language).name().to_string() };
    ui.horizontal(|ui| {
        ui.label(tl!("Interface language:"));
        egui::ComboBox::from_id_salt("interface_language").selected_text(current).width(220.0).show_ui(ui, |ui| {
            if ui.selectable_label(app.ui.language == AUTO, auto_label.as_str()).clicked() {
                let _ = app.run("ui.language", json!({"value": AUTO}));
            }
            for lang in Lang::all() {
                if ui.selectable_label(app.ui.language == lang.code(), lang.name()).clicked() {
                    let _ = app.run("ui.language", json!({"value": lang.code()}));
                }
            }
        });
    });
    ui.label(RichText::new(tl!("Automatic follows your system's language. Menus and commands change; your documents don't.")).small().weak());
}

/// Deserialize `input` over `T`'s defaults one setting at a time: a value of the wrong type is
/// dropped (its default stays) instead of failing the whole read. Objects nest (`editing`).
pub fn lenient<T: Serialize + DeserializeOwned + Default>(input: &Value) -> T {
    let mut base = serde_json::to_value(T::default()).unwrap_or(Value::Null);
    if base.is_object() {
        overlay::<T>(&mut base, &mut Vec::new(), input, 0);
    }
    serde_json::from_value(base).unwrap_or_default()
}

/// The object at `path` inside `root`.
fn object_at<'a>(root: &'a mut Value, path: &[String]) -> Option<&'a mut Map<String, Value>> {
    let mut v = root;
    for k in path {
        v = v.as_object_mut()?.get_mut(k)?;
    }
    v.as_object_mut()
}

fn overlay<T: DeserializeOwned>(root: &mut Value, path: &mut Vec<String>, input: &Value, depth: usize) {
    let Some(map) = input.as_object() else { return };
    for (k, v) in map {
        let Some(parent) = object_at(root, path) else { return };
        let old = parent.get(k).cloned();
        if depth < 4 && v.is_object() && old.as_ref().is_some_and(Value::is_object) {
            path.push(k.clone());
            overlay::<T>(root, path, v, depth + 1);
            path.pop();
            continue;
        }
        parent.insert(k.clone(), v.clone());
        if serde_json::from_value::<T>(root.clone()).is_err()
            && let Some(parent) = object_at(root, path)
        {
            match old {
                Some(o) => parent.insert(k.clone(), o),
                None => parent.remove(k),
            };
        }
    }
}

/// File ▸ Options ▸ General: make WordCraft the default app for Word documents (#295). Only where
/// the host can (`Services::make_default_app`: Windows opens Default apps, Linux and BSD use
/// `xdg-mime`); macOS shows how to do it in Finder, the web shows nothing.
fn default_app_button(app: &mut WordApp, ui: &mut Ui) {
    if app.services.make_default_app.is_some() {
        let hint = if cfg!(target_os = "windows") {
            tl!("Opens Settings › Default apps, where you choose WordCraft for .docx and the other document types.")
        } else {
            tl!("Opens .docx, .doc, .rtf and .odt files and Word templates with WordCraft from now on.")
        };
        if ui.button(tl!("Make WordCraft the default for Word documents…")).on_hover_text(hint).clicked() {
            // Straight to the hook, not through `run`: only a click may change system defaults.
            let _ = app.make_default_app();
        }
    } else if cfg!(target_os = "macos") {
        ui.label(
            egui::RichText::new(tl!("To open Word documents with WordCraft by default: select one in Finder, choose File › Get Info, pick WordCraft under Open with, then click Change All."))
                .small()
                .weak(),
        );
    }
}

/// File ▸ Options ▸ Numerals: how digits look on screen (display only, like Word's
/// File › Options › Advanced › Numeral).
fn numeral_picker(app: &mut WordApp, ui: &mut Ui) {
    use wordcraft_layout::NumeralMode;
    ui.horizontal(|ui| {
        ui.label(tl!("Numerals:"));
        let mut sel = app.session.view.numeral;
        egui::ComboBox::from_id_salt("numerals").selected_text(tl!(sel.label())).width(220.0).show_ui(ui, |ui| {
            for m in [NumeralMode::Arabic, NumeralMode::Hindi, NumeralMode::Context, NumeralMode::System] {
                ui.selectable_value(&mut sel, m, tl!(m.label()));
            }
        });
        if sel != app.session.view.numeral {
            let _ = app.run("view.numeral", json!({ "value": sel }));
        }
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;
    use serde_json::json;
    use wordcraft_geom::Unit;

    use super::{decimal, length, unit};

    use crate::theme::Appearance;
    use crate::{Services, UiState, WordApp};

    fn app() -> WordApp {
        WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Services::default())
    }

    /// File › Options settings come back after a restart, and a `ui.json` with a wrong value in
    /// one setting keeps every other setting (#485).
    #[test]
    fn options_survive_a_restart_and_junk_in_ui_json() {
        let mut a = app();
        a.run(
            "file.options",
            json!({"initials": "VK", "units": "centimeters", "typingReplacesSelection": false, "marks": {"tabs": true}, "ignoreNumbers": false, "addWords": ["zqxoptionsword"]}),
        )
        .unwrap();
        a.ui.recent_count = 30;
        a.ui.print_backgrounds = true;
        let saved = serde_json::to_vec(&a.prefs()).unwrap();
        let mut b = app();
        b.apply_prefs(UiState::from_json(&saved).unwrap());
        assert_eq!(b.session.prefs, a.session.prefs);
        assert_eq!(b.session.prefs.units, Some(Unit::Centimeters));
        assert_eq!(b.session.initials, "VK");
        assert_eq!((b.ui.recent_count, b.ui.print_backgrounds), (30, true));
        assert!(wordcraft_engine::proof::user_dictionary().contains(&"zqxoptionsword".to_string()));

        let junk = br#"{"theme": "dark", "recentCount": "lots", "printBackgrounds": 7, "optionsPane": ["x"], "dark": true,
            "editing": {"measurementUnits": "furlongs", "typingReplacesSelection": false, "marks": {"tabs": "yes", "spaces": true}}}"#;
        let ui = UiState::from_json(junk).unwrap();
        assert_eq!(ui.theme, Appearance::Dark);
        assert_eq!((ui.recent_count, ui.print_backgrounds, ui.options_pane.as_str()), (12, false, "general"));
        assert_eq!(ui.legacy_dark, Some(true));
        assert_eq!(ui.editing.units, None);
        assert!(!ui.editing.typing_replaces_selection);
        assert!(ui.editing.marks.spaces && !ui.editing.marks.tabs);
        assert!(UiState::from_json(b"[1, 2]").is_none() && UiState::from_json(b"not json").is_none());
    }

    /// Settings saved before #838 kept the unit under `editing.units` (default inches): a unit
    /// the user chose carries over to `measurementUnits`; inches, junk or a newer value don't.
    #[test]
    fn a_unit_chosen_before_regions_survives_the_rename() {
        let editing = |json: &str| UiState::from_json(format!(r#"{{"editing": {json}}}"#).as_bytes()).unwrap().editing.units;
        assert_eq!(editing(r#"{"units": "centimeters"}"#), Some(Unit::Centimeters));
        assert_eq!(editing(r#"{"units": "picas", "overtype": true}"#), Some(Unit::Picas));
        assert_eq!(editing(r#"{"units": "inches"}"#), None);
        assert_eq!(editing(r#"{"units": "furlongs"}"#), None);
        assert_eq!(editing(r#"{"units": 7}"#), None);
        assert_eq!(editing(r#"{"units": "millimeters", "measurementUnits": "points"}"#), Some(Unit::Points));
        assert_eq!(editing(r#"{"units": "millimeters", "measurementUnits": null}"#), None);
        assert_eq!(editing("{}"), None);
        let mut a = app();
        a.apply_prefs(UiState::from_json(br#"{"editing": {"units": "millimeters"}}"#).unwrap());
        assert_eq!(a.session.units(), Unit::Millimeters);
        // Saved again, it's under the new key.
        let saved = serde_json::to_value(a.prefs()).unwrap();
        assert_eq!(saved["editing"]["measurementUnits"], "millimeters");
    }

    /// `file.options` opens the page at a pane; the pane list switches panes, and their
    /// checkboxes change the settings (#485).
    #[test]
    fn options_panes_open_and_change_settings() {
        let mut a = app();
        a.run("file.options", json!({"pane": "display"})).unwrap();
        assert!(a.ui.backstage);
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        h.get_by_label("Spaces").click();
        h.run_steps(2);
        assert!(h.state().session.prefs.marks.spaces, "Display › Spaces");
        h.get_by_label("Proofing").click();
        h.run_steps(4);
        assert_eq!(h.state().ui.options_pane, "proofing");
        h.get_by_label("Ignore words in capitals").click();
        h.run_steps(2);
        assert!(!h.state().session.prefs.ignore_uppercase);
        h.get_by_label("Advanced").click();
        h.run_steps(4);
        h.get_by_label("Typing replaces selected text").click();
        h.run_steps(2);
        assert!(!h.state().session.prefs.typing_replaces_selection);
        assert!(h.query_by_label("Measurement units:").is_some());
    }

    /// File › Options › Accessibility › Animation effects: Use system setting (the default), On
    /// or Off, with a line saying what the system's own setting is while it is followed.
    #[test]
    fn animation_effects_is_an_accessibility_option() {
        use crate::theme::Animations;
        let services = Services { animations_enabled: Some(Box::new(|| Some(false))), ..Default::default() };
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), services);
        a.run("file.options", json!({"pane": "accessibility"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        assert_eq!(h.state().ui.animations, Animations::System);
        assert!(h.query_by_label("Animation effects:").is_some());
        assert!(h.query_by_label("Your system has animation effects turned off.").is_some());
        assert!(!h.state().animations_on());
        h.get_by_role(egui::accesskit::Role::ComboBox).click();
        h.run_steps(4);
        h.get_by_label("On").click();
        h.run_steps(4);
        assert_eq!(h.state().ui.animations, Animations::On);
        assert!(h.state().animations_on());
        assert!(h.query_by_label("Your system has animation effects turned off.").is_none(), "only said while the system is followed");
    }

    /// File › Options › General › Start-up (#766): the checkbox is the `ui.startScreen` setting.
    #[test]
    fn general_start_up_turns_the_start_screen_off_and_on() {
        let mut a = app();
        a.run("file.options", json!({"pane": "general"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        assert!(h.query_by_label("Start-up").is_some(), "the section is there");
        h.get_by_label("Show the Start screen when WordCraft starts").click();
        h.run_steps(4);
        assert!(!h.state().ui.start_screen, "the checkbox turns it off");
        h.get_by_label("Show the Start screen when WordCraft starts").click();
        h.run_steps(4);
        assert!(h.state().ui.start_screen, "and on again");
    }

    /// File › Options › Advanced › Animation: the checkbox is the `ui.caretAnimation` setting,
    /// and the section is translated.
    #[test]
    fn advanced_animation_turns_the_caret_animation_off_and_on() {
        let mut a = app();
        a.run("file.options", json!({"pane": "advanced"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        assert!(h.query_by_label("Animation").is_some(), "the section is there");
        h.get_by_label("Animate the caret (it glides and fades)").click();
        h.run_steps(4);
        assert!(!h.state().ui.caret_animation, "the checkbox turns it off");
        h.get_by_label("Animate the caret (it glides and fades)").click();
        h.run_steps(4);
        assert!(h.state().ui.caret_animation, "and on again");
        h.state_mut().run("ui.language", json!({"value": "es"})).unwrap();
        h.run_steps(4);
        assert!(h.query_by_label("Animación").is_some() && h.query_by_label("Animation").is_none());
    }

    /// #838: in a Polish region the Page Setup margins read `2,54 cm` (the region's unit and
    /// decimal comma), a typed `3,5` or `1 in` is understood, and the Options page shows the
    /// region's unit until the user picks another.
    #[test]
    fn length_fields_follow_the_region() {
        let mut a = app();
        a.run("file.options", json!({"region": "pl-PL"})).unwrap();
        a.run("file.new", json!({})).unwrap();
        assert_eq!(wordcraft_geom::paper_name(a.session.doc.last_section.page_w, a.session.doc.last_section.page_h), Some("A4"));
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(2);
        // The dialog reads the unit when it opens, after a frame has picked up the region.
        h.state_mut().run("ui.dialog", json!({"name": "pageSetup"})).unwrap();
        h.run_steps(4);
        assert_eq!((unit(), decimal()), (Unit::Centimeters, ','));
        assert_eq!(length(72.0), "2,54 cm");
        assert_eq!(h.query_all_by_value("2,54 cm").count(), 4, "four margins in centimetres with a decimal comma");

        h.state_mut().run("file.options", json!({"units": "inches"})).unwrap();
        h.run_steps(4);
        assert_eq!(length(108.0), "1,5\"", "the user's unit, still the region's decimal comma");
        h.state_mut().run("file.options", json!({"units": "auto", "region": "en-US"})).unwrap();
        h.run_steps(4);
        assert_eq!(length(72.0), "1\"");
    }
}
