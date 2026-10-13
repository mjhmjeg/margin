//! Margin's macOS skin: flat Mac greys, the system blue accent, and one interface type size
//! (13 pt, regular weight). Hooks in `theme.rs` (tokens, font helpers) and `chrome.rs` (title bar)
//! consult [`on`]; everything else reads the tokens as before.

use std::sync::atomic::{AtomicBool, Ordering};

use egui::Color32;

use crate::theme::Tokens;

// ponytail: process-wide switch, read by `Tokens::get` which only has a ctx. Becomes a user
// setting (File › Options › Interface style) if this goes upstream.
static ON: AtomicBool = AtomicBool::new(true);

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Whether the one-size type rule applies here: the skin is on and we're not inside the ribbon.
pub fn flat_type() -> bool {
    on() && !CLASSIC.with(std::cell::Cell::get)
}

thread_local! {
    static CLASSIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While alive, the font helpers give upstream's sizes and weights (colours stay Margin's): the Classic ribbon is laid out
/// for its own type sizes, and 13 pt everywhere would clip its labels.
pub struct Classic(bool);

impl Classic {
    pub fn enter() -> Classic {
        Classic(CLASSIC.with(|c| c.replace(true)))
    }

    /// [`Classic::enter`] for a whole `ui`, also restoring upstream's default text styles there
    /// (`theme::apply` set them while the skin was on).
    pub fn enter_ui(ui: &mut egui::Ui) -> Classic {
        let guard = Classic::enter();
        let s = ui.style_mut();
        s.text_styles.insert(egui::TextStyle::Body, crate::theme::regular(12.5));
        s.text_styles.insert(egui::TextStyle::Button, crate::theme::regular(12.5));
        s.text_styles.insert(egui::TextStyle::Small, crate::theme::regular(10.5));
        s.text_styles.insert(egui::TextStyle::Heading, crate::theme::semibold(17.0));
        guard
    }
}

impl Drop for Classic {
    fn drop(&mut self) {
        CLASSIC.with(|c| c.set(self.0));
    }
}

pub fn set(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// The product name shown in the window title and menus.
pub const APP_NAME: &str = "Margin";

/// The one interface type size, in points.
pub const UI_SIZE: f32 = 13.0;

/// Apple's system blue, and its pressed/hover shade (also used for blue text).
pub const ACCENT: Color32 = Color32::from_rgb(0x0A, 0x64, 0xD6);
pub const ACCENT_INK: Color32 = Color32::from_rgb(0x08, 0x50, 0xA8);
/// Dark appearance: `accent_text` doubles as blue text on dark chrome and as the hover fill under
/// `on_accent`, so it must be light; Apple's #0A84FF gives white text only 3.65:1 (upstream wants
/// 4.5:1, #655). A lighter blue with near-black text on it passes both (6.1:1, 6.6:1).
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x3D, 0x9B, 0xFF);
pub const ACCENT_DARK_INK: Color32 = Color32::from_rgb(0x6C, 0xB4, 0xFF);
pub const ON_ACCENT_DARK: Color32 = Color32::from_rgb(0x0B, 0x1A, 0x2E);

/// The size the font helpers use: every interface size becomes [`UI_SIZE`] when the skin is on.
pub fn size(requested: f32) -> f32 {
    if flat_type() { UI_SIZE } else { requested }
}

/// Mac light palette over upstream's `base`; colours not listed keep upstream's values.
pub fn light(base: Tokens) -> Tokens {
    if !on() {
        return base;
    }
    let g = Color32::from_gray;
    Tokens {
        title_bar: g(0xF6),
        ribbon: g(0xF6),
        tab_strip: g(0xF6),
        canvas: g(0xEC),
        panel: g(0xFA),
        border: g(0xE3),
        border_strong: g(0xD1),
        text: Color32::from_rgb(0x1D, 0x1D, 0x1F),
        text_dim: Color32::from_rgb(0x6E, 0x6E, 0x73),
        text_disabled: Color32::from_rgb(0xB8, 0xB8, 0xBC),
        icon: Color32::from_rgb(0x3C, 0x3C, 0x43),
        hover: g(0xEA),
        pressed: g(0xDC),
        checked: g(0xE3),
        accent: ACCENT,
        accent_text: ACCENT_INK,
        on_accent: Color32::WHITE,
        input: Color32::WHITE,
        // 3:1 against field and chrome (#657); Mac's own hairline (#D1D1D1) is too faint.
        input_border: g(0x8A),
        group_label: Color32::from_rgb(0xA1, 0xA1, 0xA6),
        ruler: Color32::WHITE,
        ruler_margin: g(0xE3),
        ruler_tick: Color32::from_rgb(0x8E, 0x8E, 0x93),
        status_bar: g(0xF6),
        selection: Color32::from_rgba_unmultiplied(0x0A, 0x64, 0xD6, 0x40),
        page_shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 0x1A),
        blue: ACCENT,
        ..base
    }
}

/// Mac dark palette over upstream's `base`.
pub fn dark(base: Tokens) -> Tokens {
    if !on() {
        return base;
    }
    let g = Color32::from_gray;
    Tokens {
        title_bar: g(0x2A),
        ribbon: g(0x2A),
        tab_strip: g(0x2A),
        canvas: g(0x1E),
        panel: g(0x26),
        border: g(0x38),
        border_strong: g(0x47),
        text: g(0xF2),
        text_dim: Color32::from_rgb(0xA1, 0xA1, 0xA6),
        text_disabled: Color32::from_rgb(0x6E, 0x6E, 0x73),
        icon: g(0xDD),
        hover: g(0x33),
        pressed: g(0x3D),
        checked: g(0x3A),
        accent: ACCENT_DARK,
        accent_text: ACCENT_DARK_INK,
        on_accent: ON_ACCENT_DARK,
        input: g(0x1E),
        input_border: g(0x7A),
        group_label: Color32::from_rgb(0x6E, 0x6E, 0x73),
        ruler: g(0x2A),
        ruler_margin: g(0x38),
        ruler_tick: Color32::from_rgb(0x8E, 0x8E, 0x93),
        status_bar: g(0x2A),
        selection: Color32::from_rgba_unmultiplied(0x3D, 0x9B, 0xFF, 0x55),
        blue: ACCENT_DARK,
        ..base
    }
}

// ---------------------------------------------------------------------------------------------
// Margin tokens (Claude Design, 2026-10-13: docs/design-handoff/claude-design/tokens.json).
// Upstream's `Tokens` keeps chrome, panel, canvas, borders, text, hover/pressed/checked and the
// accent; these are the ones Margin's own screens add.

/// A drop shadow (CSS `x y blur` + colour), in pt.
pub type Shadow = egui::epaint::Shadow;

const fn shadow(y: i8, blur: u8, color: Color32) -> Shadow {
    Shadow { offset: [0, y], blur, spread: 0, color }
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied_const(r, g, b, (a * 255.0) as u8)
}

/// Person and assistant colours **on the page**, which stays white in both appearances.
pub const AUTHOR_1_PAGE: Color32 = Color32::from_rgb(0xB8, 0x32, 0x7A);
pub const ASSISTANT_PAGE: Color32 = Color32::from_rgb(0x5B, 0x4F, 0xCF);
/// More people, in order after `author_1` (page colours; all ≥ 4.5:1 on white).
pub const AUTHOR_ROTATION: [Color32; 4] = [
    Color32::from_rgb(0x1F, 0x7A, 0x3A),
    Color32::from_rgb(0xB4, 0x53, 0x09),
    Color32::from_rgb(0x0E, 0x74, 0x90),
    Color32::from_rgb(0x9D, 0x17, 0x4D),
];
/// Tinted background behind an assistant's / a person's comment anchor (both appearances).
pub const ASSISTANT_TINT: Color32 = rgba(91, 79, 207, 0.09);
pub const PERSON_TINT: Color32 = rgba(184, 50, 122, 0.12);

#[derive(Clone, Copy, Debug)]
pub struct MarginTokens {
    pub dark: bool,
    /// Inspector and dialog section titles (replaces upstream's `group_label` there: 2.5:1 failed).
    pub section_title: Color32,
    /// Helper sentences under controls.
    pub hint: Color32,
    /// Segmented-control and switch-off track; the selected segment's chip and its shadow.
    pub segment_track: Color32,
    pub segment_chip: Color32,
    pub chip_shadow: [Shadow; 2],
    /// Fields, popup buttons, secondary buttons.
    pub field_bg: Color32,
    /// 3 pt outside a focused control.
    pub focus_ring: Color32,
    /// Page edge hairline (0.5 pt) and drop shadows, replacing the flat page border.
    pub page_edge: Color32,
    pub page_shadow: [Shadow; 2],
    /// Popovers, menus, dialogs.
    pub popover_bg: Color32,
    pub popover_border: Color32,
    pub popover_shadow: Shadow,
    /// Author colours for panels (review list, legends); the page uses the `*_PAGE` constants.
    pub author_1: Color32,
    /// Reserved for AI authors, never assigned to a person.
    pub assistant: Color32,
}

impl MarginTokens {
    pub fn light() -> Self {
        MarginTokens {
            dark: false,
            section_title: Color32::from_rgb(0x1D, 0x1D, 0x1F),
            hint: Color32::from_rgb(0x6E, 0x6E, 0x73),
            segment_track: Color32::from_rgb(0xF3, 0xF3, 0xF5),
            segment_chip: Color32::WHITE,
            chip_shadow: [shadow(0, 0, rgba(0, 0, 0, 0.14)), shadow(1, 2, rgba(0, 0, 0, 0.08))],
            field_bg: Color32::WHITE,
            focus_ring: rgba(10, 100, 214, 0.45),
            page_edge: rgba(0, 0, 0, 0.10),
            // ponytail: CSS had three layers; the hairline is `page_edge`, the other two are here.
            page_shadow: [shadow(1, 3, rgba(0, 0, 0, 0.10)), shadow(8, 24, rgba(0, 0, 0, 0.06))],
            popover_bg: Color32::WHITE,
            popover_border: Color32::from_gray(0xD1),
            popover_shadow: shadow(8, 24, rgba(0, 0, 0, 0.14)),
            author_1: AUTHOR_1_PAGE,
            assistant: ASSISTANT_PAGE,
        }
    }

    pub fn dark() -> Self {
        MarginTokens {
            dark: true,
            section_title: Color32::from_gray(0xF2),
            hint: Color32::from_rgb(0x98, 0x98, 0x9D),
            segment_track: Color32::from_gray(0x1E),
            segment_chip: Color32::from_rgb(0x4A, 0x4A, 0x4C),
            chip_shadow: [shadow(0, 0, rgba(255, 255, 255, 0.06)), shadow(1, 2, rgba(0, 0, 0, 0.5))],
            field_bg: Color32::from_gray(0x1E),
            focus_ring: rgba(61, 155, 255, 0.50),
            page_edge: rgba(0, 0, 0, 0.6),
            page_shadow: [shadow(2, 10, rgba(0, 0, 0, 0.45)), shadow(0, 0, Color32::TRANSPARENT)],
            popover_bg: Color32::from_gray(0x2E),
            popover_border: Color32::from_gray(0x47),
            popover_shadow: shadow(8, 24, rgba(0, 0, 0, 0.50)),
            author_1: Color32::from_rgb(0xEC, 0x86, 0xBC),
            assistant: Color32::from_rgb(0xAD, 0xA3, 0xFF),
        }
    }

    pub fn get(ctx: &egui::Context) -> Self {
        if ctx.global_style().visuals.dark_mode { Self::dark() } else { Self::light() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_flattens_type_and_recolours() {
        // ponytail: never flips the global off here; tests run in parallel and share it.
        assert!(on());
        assert_eq!(size(11.0), UI_SIZE);
        assert_eq!(size(17.0), UI_SIZE);
        assert_eq!(Tokens::light().accent, ACCENT);
        assert_eq!(Tokens::dark().accent, ACCENT_DARK);
    }

    /// WCAG contrast ratio.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// Claude Design's pairs (HANDOFF.md §1), in both appearances: text ≥ 4.5:1, field outlines ≥ 3:1.
    #[test]
    fn margin_tokens_meet_contrast_floors() {
        for (t, m) in [(Tokens::light(), MarginTokens::light()), (Tokens::dark(), MarginTokens::dark())] {
            let text = [
                ("section_title on panel", m.section_title, t.panel),
                ("text_dim on panel", t.text_dim, t.panel),
                ("text_dim on segment_track", t.text_dim, m.segment_track),
                ("hint on panel", m.hint, t.panel),
                ("text on segment_chip", t.text, m.segment_chip),
                ("on_accent on accent", t.on_accent, t.accent),
                ("accent link on panel", t.accent, t.panel),
                ("author_1 on panel", m.author_1, t.panel),
                ("assistant on panel", m.assistant, t.panel),
                ("text on popover", t.text, m.popover_bg),
            ];
            for (name, fg, bg) in text {
                let r = contrast(fg, bg);
                assert!(r >= 4.5, "dark={}: {name} is {r:.2}:1", t.dark);
            }
            let r = contrast(t.input_border, m.field_bg);
            assert!(r >= 3.0, "dark={}: input_border on field_bg is {r:.2}:1", t.dark);
        }
        // On the page (white in both appearances).
        for (name, c) in
            [("author_1", AUTHOR_1_PAGE), ("assistant", ASSISTANT_PAGE)].into_iter().chain(AUTHOR_ROTATION.iter().map(|c| ("rotation", *c)))
        {
            let r = contrast(c, Color32::WHITE);
            assert!(r >= 4.5, "{name} on the page is {r:.2}:1");
        }
    }

    #[test]
    fn new_windows_follow_the_mac_appearance() {
        assert_eq!(crate::UiState::default().theme, crate::theme::Appearance::System);
    }
}
