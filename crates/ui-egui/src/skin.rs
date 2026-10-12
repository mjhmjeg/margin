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

    #[test]
    fn new_windows_follow_the_mac_appearance() {
        assert_eq!(crate::UiState::default().theme, crate::theme::Appearance::System);
    }
}
