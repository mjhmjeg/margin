//! O4 (OPTIMO.md §5 T4): Looks — named sets of paragraph styles (Title, Subtitle, Heading 1–3,
//! Body, List Bullet/Number, Quote, Header/Footer) that Format for me writes to the document's
//! style table. Four presets of our own (open fonts, our colours), and looks people save from a
//! document they like ("Save as my look…", stored by the app as JSON).
//!
//! Every style in a look sets the same fields explicitly ([`LookStyle`]), so switching from one
//! look to another leaves nothing of the first behind (`styles.modify` overlays).

use serde::{Deserialize, Serialize};
use wordcraft_doc::props::{CharProps, LineSpacing, ParaProps, Rgb, TextColor};
use wordcraft_doc::{Align, Document};

/// The presets, in the switcher's order.
pub const PRESETS: [&str; 4] = ["clean", "classic", "report", "letter"];

/// The styles a look sets, by name (the name `styles.modify` and `styles.create` take).
pub const STYLES: [&str; 11] =
    ["Normal", "Title", "Subtitle", "Heading 1", "Heading 2", "Heading 3", "List Bullet", "List Number", "Quote", "Header", "Footer"];

/// Most saved looks, and the longest name.
pub const MAX_SAVED: usize = 24;
pub const MAX_NAME: usize = 40;

/// Open families only (CLAUDE.md fonts rule): both ship with Optimo (`craft-fonts`).
const SANS: &str = "Source Sans 3";
const SERIF: &str = "Source Serif 4";

/// One style of a look: the fields every look sets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LookStyle {
    pub style: String,
    pub font: String,
    pub size: f32,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub caps: bool,
    /// "RRGGBB", or empty for automatic.
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub before: f32,
    #[serde(default)]
    pub after: f32,
    /// Line spacing as a multiple of single.
    #[serde(default = "one")]
    pub line: f32,
    #[serde(default)]
    pub center: bool,
}

fn one() -> f32 {
    1.0
}

/// A look: its name and its styles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub name: String,
    pub styles: Vec<LookStyle>,
}

impl LookStyle {
    /// As `styles.modify` takes it: `{"style", "chr", "para"}`.
    pub fn modify_params(&self) -> serde_json::Value {
        let color = Rgb::parse(&self.color).map_or(TextColor::Auto, TextColor::Rgb);
        let chr = CharProps {
            font: Some(self.font.clone()),
            font_cs: Some(self.font.clone()),
            size: Some(self.size),
            bold: Some(self.bold),
            bold_cs: Some(self.bold),
            italic: Some(self.italic),
            caps: Some(self.caps),
            color: Some(color),
            ..Default::default()
        };
        let para = ParaProps {
            space_before: Some(self.before),
            space_after: Some(self.after),
            line_spacing: Some(LineSpacing::Multiple(self.line)),
            align: Some(if self.center { Align::Center } else { Align::Left }),
            keep_next: Some(self.style.starts_with("Heading") || self.style == "Title" || self.style == "Subtitle"),
            ..Default::default()
        };
        serde_json::json!({"style": self.style, "chr": chr, "para": para})
    }
}

impl Look {
    /// A preset by id ([`PRESETS`]).
    pub fn preset(id: &str) -> Option<Look> {
        // (style, font, size, bold, italic, caps, color, before, after, line, center)
        type Row = (&'static str, &'static str, f32, bool, bool, bool, &'static str, f32, f32, f32, bool);
        let rows: [Row; 11] = match id {
            // Clean: one sans family, dark ink, a teal accent on the headings.
            "clean" => [
                ("Normal", SANS, 11.0, false, false, false, "", 0.0, 8.0, 1.2, false),
                ("Title", SANS, 26.0, true, false, false, "1E2A32", 0.0, 4.0, 1.0, false),
                ("Subtitle", SANS, 14.0, false, false, false, "56646E", 0.0, 16.0, 1.0, false),
                ("Heading 1", SANS, 17.0, true, false, false, "1E5F66", 20.0, 6.0, 1.0, false),
                ("Heading 2", SANS, 13.5, true, false, false, "1E5F66", 14.0, 4.0, 1.0, false),
                ("Heading 3", SANS, 11.5, true, false, false, "1E2A32", 10.0, 2.0, 1.0, false),
                ("List Bullet", SANS, 11.0, false, false, false, "", 0.0, 3.0, 1.2, false),
                ("List Number", SANS, 11.0, false, false, false, "", 0.0, 3.0, 1.2, false),
                ("Quote", SANS, 11.0, false, true, false, "56646E", 6.0, 10.0, 1.2, false),
                ("Header", SANS, 9.0, false, false, false, "56646E", 0.0, 0.0, 1.0, false),
                ("Footer", SANS, 9.0, false, false, false, "56646E", 0.0, 0.0, 1.0, true),
            ],
            // Classic: one serif family, black, centred title.
            "classic" => [
                ("Normal", SERIF, 11.5, false, false, false, "", 0.0, 8.0, 1.25, false),
                ("Title", SERIF, 28.0, false, false, false, "111111", 0.0, 6.0, 1.0, true),
                ("Subtitle", SERIF, 14.0, false, true, false, "444444", 0.0, 18.0, 1.0, true),
                ("Heading 1", SERIF, 18.0, true, false, false, "111111", 22.0, 6.0, 1.0, false),
                ("Heading 2", SERIF, 14.0, true, false, false, "111111", 14.0, 4.0, 1.0, false),
                ("Heading 3", SERIF, 12.0, true, true, false, "111111", 10.0, 2.0, 1.0, false),
                ("List Bullet", SERIF, 11.5, false, false, false, "", 0.0, 3.0, 1.25, false),
                ("List Number", SERIF, 11.5, false, false, false, "", 0.0, 3.0, 1.25, false),
                ("Quote", SERIF, 11.5, false, true, false, "444444", 6.0, 10.0, 1.25, false),
                ("Header", SERIF, 9.0, false, true, false, "444444", 0.0, 0.0, 1.0, true),
                ("Footer", SERIF, 9.0, false, false, false, "444444", 0.0, 0.0, 1.0, true),
            ],
            // Report: sans headings in deep blue over serif body text, caps for level 3.
            "report" => [
                ("Normal", SERIF, 11.0, false, false, false, "", 0.0, 8.0, 1.2, false),
                ("Title", SANS, 30.0, true, false, false, "173B66", 0.0, 4.0, 1.0, false),
                ("Subtitle", SANS, 15.0, false, false, false, "4A5A6E", 0.0, 20.0, 1.0, false),
                ("Heading 1", SANS, 18.0, true, false, false, "173B66", 24.0, 6.0, 1.0, false),
                ("Heading 2", SANS, 14.0, true, false, false, "2B5C94", 16.0, 4.0, 1.0, false),
                ("Heading 3", SANS, 10.5, true, false, true, "4A5A6E", 12.0, 2.0, 1.0, false),
                ("List Bullet", SERIF, 11.0, false, false, false, "", 0.0, 3.0, 1.2, false),
                ("List Number", SERIF, 11.0, false, false, false, "", 0.0, 3.0, 1.2, false),
                ("Quote", SERIF, 11.0, false, true, false, "4A5A6E", 6.0, 10.0, 1.2, false),
                ("Header", SANS, 8.5, false, false, true, "4A5A6E", 0.0, 0.0, 1.0, false),
                ("Footer", SANS, 8.5, false, false, false, "4A5A6E", 0.0, 0.0, 1.0, true),
            ],
            // Letter: quiet serif, headings barely larger than the text, tight spacing.
            "letter" => [
                ("Normal", SERIF, 11.0, false, false, false, "", 0.0, 10.0, 1.15, false),
                ("Title", SERIF, 16.0, true, false, false, "111111", 0.0, 2.0, 1.0, false),
                ("Subtitle", SERIF, 11.0, false, true, false, "444444", 0.0, 14.0, 1.0, false),
                ("Heading 1", SERIF, 12.5, true, false, false, "111111", 14.0, 4.0, 1.0, false),
                ("Heading 2", SERIF, 11.0, true, false, false, "111111", 10.0, 2.0, 1.0, false),
                ("Heading 3", SERIF, 11.0, false, true, false, "111111", 8.0, 2.0, 1.0, false),
                ("List Bullet", SERIF, 11.0, false, false, false, "", 0.0, 2.0, 1.15, false),
                ("List Number", SERIF, 11.0, false, false, false, "", 0.0, 2.0, 1.15, false),
                ("Quote", SERIF, 11.0, false, true, false, "444444", 4.0, 10.0, 1.15, false),
                ("Header", SERIF, 9.0, false, false, false, "444444", 0.0, 0.0, 1.0, false),
                ("Footer", SERIF, 9.0, false, false, false, "444444", 0.0, 0.0, 1.0, true),
            ],
            _ => return None,
        };
        let styles = rows
            .iter()
            .map(|&(style, font, size, bold, italic, caps, color, before, after, line, center)| LookStyle {
                style: style.into(),
                font: font.into(),
                size,
                bold,
                italic,
                caps,
                color: color.into(),
                before,
                after,
                line,
                center,
            })
            .collect();
        Some(Look { name: id.into(), styles })
    }

    /// The preset's name in the switcher.
    pub fn label(id: &str) -> &str {
        match id {
            "clean" => "Clean",
            "classic" => "Classic",
            "report" => "Report",
            "letter" => "Letter",
            other => other,
        }
    }

    /// The look `doc` has now, under `name`: each of [`STYLES`] as it resolves (its own fields,
    /// else its base styles', else the document defaults). Styles the document lacks are left out.
    pub fn from_doc(doc: &Document, name: &str) -> Look {
        let st = &doc.styles;
        let styles = STYLES
            .iter()
            .filter_map(|n| {
                let style = st.find(n)?;
                let chain = st.chain(&style.id);
                let chr = |f: &dyn Fn(&CharProps) -> Option<f32>| chain.iter().rev().find_map(|x| f(&x.chr));
                let flag = |f: &dyn Fn(&CharProps) -> Option<bool>| chain.iter().rev().find_map(|x| f(&x.chr)).unwrap_or(false);
                let para = |f: &dyn Fn(&ParaProps) -> Option<f32>| chain.iter().rev().find_map(|x| f(&x.para));
                let font =
                    chain.iter().rev().find_map(|x| x.chr.font.clone()).or_else(|| st.default_chr.font.clone()).unwrap_or_else(|| SERIF.into());
                let color = match chain.iter().rev().find_map(|x| x.chr.color) {
                    Some(TextColor::Rgb(c)) => format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2),
                    _ => String::new(),
                };
                let line = match chain.iter().rev().find_map(|x| x.para.line_spacing).or(st.default_para.line_spacing) {
                    Some(LineSpacing::Multiple(m)) => m,
                    _ => 1.0,
                };
                Some(LookStyle {
                    style: (*n).into(),
                    font,
                    size: chr(&|c| c.size).or(st.default_chr.size).unwrap_or(11.0),
                    bold: flag(&|c| c.bold),
                    italic: flag(&|c| c.italic),
                    caps: flag(&|c| c.caps),
                    color,
                    before: para(&|p| p.space_before).or(st.default_para.space_before).unwrap_or(0.0),
                    after: para(&|p| p.space_after).or(st.default_para.space_after).unwrap_or(0.0),
                    line,
                    center: chain.iter().rev().find_map(|x| x.para.align) == Some(Align::Center),
                })
            })
            .collect();
        Look { name: clip(name), styles }.sane()
    }

    /// Bounded and valid, whatever a file said: a short name, known styles only (once each),
    /// short font names, sizes and spacing in range.
    pub fn sane(mut self) -> Look {
        self.name = clip(&self.name);
        let mut seen = Vec::new();
        self.styles.retain(|s| {
            let ok = STYLES.contains(&s.style.as_str()) && !seen.contains(&s.style);
            seen.push(s.style.clone());
            ok
        });
        for s in &mut self.styles {
            s.font = s.font.chars().take(64).collect::<String>().trim().to_string();
            if s.font.is_empty() {
                s.font = SERIF.into();
            }
            let fin = |x: f32, lo: f32, hi: f32, d: f32| if x.is_finite() { x.clamp(lo, hi) } else { d };
            s.size = fin(s.size, 4.0, 96.0, 11.0);
            s.before = fin(s.before, 0.0, 120.0, 0.0);
            s.after = fin(s.after, 0.0, 120.0, 0.0);
            s.line = fin(s.line, 0.5, 4.0, 1.0);
            if Rgb::parse(&s.color).is_none() {
                s.color.clear();
            }
        }
        self
    }
}

fn clip(name: &str) -> String {
    let n: String = name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    if n.is_empty() { "My look".into() } else { n }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_set_every_style_with_open_fonts() {
        for id in PRESETS {
            let l = Look::preset(id).unwrap();
            let names: Vec<&str> = l.styles.iter().map(|s| s.style.as_str()).collect();
            assert_eq!(names, STYLES, "{id}");
            assert!(l.styles.iter().all(|s| s.font == SANS || s.font == SERIF), "{id}");
            assert_eq!(l.clone().sane(), l, "{id}: presets are already sane");
        }
        assert!(Look::preset("Calibri").is_none());
    }

    #[test]
    fn a_hostile_saved_look_is_made_sane() {
        let bad = Look {
            name: "\u{7}x".repeat(100),
            styles: vec![
                LookStyle { style: "Evil".into(), ..Look::preset("clean").unwrap().styles[0].clone() },
                LookStyle {
                    size: f32::NAN,
                    before: -5.0,
                    line: 99.0,
                    color: "zz".into(),
                    font: " ".into(),
                    ..Look::preset("clean").unwrap().styles[1].clone()
                },
            ],
        };
        let l = bad.sane();
        assert_eq!(l.name.chars().count(), MAX_NAME);
        assert_eq!(l.styles.len(), 1);
        let t = &l.styles[0];
        assert_eq!((t.size, t.before, t.line, t.color.as_str(), t.font.as_str()), (11.0, 0.0, 4.0, "", SERIF));
    }
}
