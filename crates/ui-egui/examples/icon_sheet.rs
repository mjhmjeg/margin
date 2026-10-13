//! Icon sheet: every Margin handoff icon at 17 pt and 16 pt, light and dark, rendered offscreen.
//!
//! `cargo run -p wordcraft-ui-egui --example icon_sheet -- docs/mac/icons`
//! writes `sheet-light.png` and `sheet-dark.png` into the folder (relative to the cwd).

use egui::{Align2, Color32, FontId, Rect, pos2, vec2};

const NAMES: [&str; 38] = [
    "accept",
    "alignCenter",
    "justify",
    "alignLeft",
    "alignRight",
    "assistant",
    "chevronDown",
    "chevronRight",
    "chevronUp",
    "comment",
    "export",
    "focus",
    "picture",
    "inspector",
    "link",
    "lock",
    "more",
    "newDocument",
    "open",
    "popup",
    "print",
    "recent",
    "reject",
    "resolve",
    "review",
    "saveAs",
    "search",
    "send",
    "settings",
    "shapes",
    "sidebarLeft",
    "stop",
    "table",
    "trackChanges",
    "viewGrid",
    "viewList",
    "viewPageless",
    "viewPages",
];

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "docs/mac/icons".into());
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("mkdir {dir}: {e}");
        return;
    }
    for (_dark, file, bg, ink, dim) in [
        (false, "sheet-light.png", Color32::from_rgb(0xFA, 0xFA, 0xFA), Color32::from_rgb(0x1D, 0x1D, 0x1F), Color32::from_rgb(0x6E, 0x6E, 0x73)),
        (true, "sheet-dark.png", Color32::from_rgb(0x1E, 0x1E, 0x1E), Color32::from_rgb(0xF2, 0xF2, 0xF2), Color32::from_rgb(0x98, 0x98, 0x9D)),
    ] {
        let cols = 7.0;
        let (cw, ch) = (140.0, 74.0);
        let size = vec2(cols * cw + 20.0, (NAMES.len() as f32 / cols).ceil() * ch + 20.0);
        let mut harness = egui_kittest::Harness::builder().with_size(size).with_pixels_per_point(3.0).wgpu().build_ui(move |ui| {
            let p = ui.painter().clone();
            p.rect_filled(ui.max_rect(), 0.0, bg);
            for (i, name) in NAMES.iter().enumerate() {
                let (col, row) = ((i % 7) as f32, (i / 7) as f32);
                let o = pos2(10.0 + col * cw, 10.0 + row * ch);
                let c17 = Rect::from_center_size(o + vec2(26.0, 24.0), vec2(17.0, 17.0));
                let c16 = Rect::from_center_size(o + vec2(66.0, 24.0), vec2(16.0, 16.0));
                let c32 = Rect::from_center_size(o + vec2(104.0, 24.0), vec2(32.0, 32.0));
                wordcraft_ui_egui::icons::paint(&p, c17, name, ink, ink);
                wordcraft_ui_egui::icons::paint(&p, c16, name, dim, dim);
                wordcraft_ui_egui::icons::paint(&p, c32, name, ink, ink);
                p.text(o + vec2(65.0, 54.0), Align2::CENTER_CENTER, name, FontId::proportional(10.0), dim);
            }
        });
        harness.run();
        match harness.render() {
            Ok(img) => match img.save(format!("{dir}/{file}")) {
                Ok(()) => println!("{dir}/{file}"),
                Err(e) => eprintln!("save: {e}"),
            },
            Err(e) => eprintln!("render failed: {e}"),
        }
    }
}
