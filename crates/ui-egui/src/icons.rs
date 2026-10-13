//! WordCraft's icon set, drawn in code (original artwork; no external icon assets).
//!
//! Icons are designed on a 20×20 grid in two colours: line work (`c`) and an accent (`a`).
//!
//! The Margin handoff set (HANDOFF §11) lives in [`handoff`]: one colour, 24 grid, stroke 1.5,
//! round caps and joins. Names that exist in both sets draw the handoff shape.

use egui::{Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    c: Color32,
    a: Color32,
    w: f32,
}

impl Pen<'_> {
    fn s(&self) -> f32 {
        self.r.width().min(self.r.height()) / 20.0
    }
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        let s = self.s();
        let o = self.r.center() - vec2(10.0 * s, 10.0 * s);
        pos2(o.x + x * s, o.y + y * s)
    }
    fn stroke(&self, c: Color32) -> Stroke {
        Stroke::new(self.w * self.s(), c)
    }
    fn line(&self, pts: &[(f32, f32)]) {
        self.line_c(pts, self.c);
    }
    fn line_c(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::line(v, self.stroke(c)));
    }
    fn closed(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::closed_line(v, self.stroke(c)));
    }
    fn fill(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::convex_polygon(v, c, Stroke::NONE));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.p.rect_stroke(Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1)), 1.0, self.stroke(c), egui::StrokeKind::Middle);
    }
    fn frect(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.p.rect_filled(Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1)), 0.5, c);
    }
    fn circle(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_stroke(self.pt(x, y), r * self.s(), self.stroke(c));
    }
    fn fcircle(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_filled(self.pt(x, y), r * self.s(), c);
    }
    fn text(&self, x: f32, y: f32, size: f32, t: &str, c: Color32, bold: bool) {
        let font = if bold { crate::theme::semibold(size * self.s()) } else { FontId::proportional(size * self.s()) };
        self.p.text(self.pt(x, y), egui::Align2::CENTER_CENTER, t, font, c);
    }
    /// Horizontal text lines (a paragraph).
    fn lines(&self, x0: f32, x1: f32, ys: &[f32]) {
        for y in ys {
            self.line(&[(x0, *y), (x1, *y)]);
        }
    }
    /// A page outline with a folded corner.
    fn page(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.closed(&[(x0, y0), (x1 - 4.0, y0), (x1, y0 + 4.0), (x1, y1), (x0, y1)], self.c);
        self.line(&[(x1 - 4.0, y0), (x1 - 4.0, y0 + 4.0), (x1, y0 + 4.0)]);
    }
    fn arrow_down(&self, x: f32, y: f32, c: Color32) {
        self.fill(&[(x - 2.5, y - 1.2), (x + 2.5, y - 1.2), (x, y + 1.5)], c);
    }
}

/// Paint icon `name` into `r`. `c` is the line colour; the accent comes from the theme.
pub fn paint(p: &Painter, r: Rect, name: &str, c: Color32, accent: Color32) {
    if let Some(items) = handoff::shapes(name) {
        handoff::paint(p, r, items, c);
        return;
    }
    let pen = Pen { p, r, c, a: accent, w: 1.25 };
    let a = accent;
    let red = Color32::from_rgb(0xD1, 0x3B, 0x3B);
    let yellow = Color32::from_rgb(0xF2, 0xC8, 0x11);
    let green = Color32::from_rgb(0x2E, 0x8B, 0x57);
    let orange = Color32::from_rgb(0xE0, 0x7B, 0x1F);
    match name {
        // Clipboard
        "paste" => {
            pen.rect(4.0, 3.5, 14.0, 17.0, c);
            pen.frect(7.0, 2.0, 11.0, 5.0, a);
            pen.frect(9.0, 8.0, 17.5, 18.5, pen.a.linear_multiply(0.18));
            pen.rect(9.0, 8.0, 17.5, 18.5, a);
            pen.lines(10.8, 15.8, &[11.0, 13.5, 16.0]);
        }
        "cut" => {
            pen.circle(6.0, 15.0, 2.5, c);
            pen.circle(14.0, 15.0, 2.5, c);
            pen.line(&[(7.5, 13.0), (14.0, 3.0)]);
            pen.line(&[(12.5, 13.0), (6.0, 3.0)]);
        }
        "copy" => {
            pen.rect(3.5, 3.0, 12.5, 14.0, c);
            pen.frect(7.5, 6.5, 16.5, 17.5, Color32::TRANSPARENT);
            pen.rect(7.5, 6.5, 16.5, 17.5, a);
        }
        "painter" => {
            pen.frect(4.0, 3.0, 16.0, 8.0, yellow);
            pen.rect(4.0, 3.0, 16.0, 8.0, c);
            pen.line(&[(16.0, 5.5), (17.5, 5.5), (17.5, 10.0), (10.0, 10.0), (10.0, 12.0)]);
            pen.frect(8.8, 12.0, 11.2, 18.0, c);
        }
        // Font
        "bold" => pen.text(10.0, 10.5, 15.0, "B", c, true),
        "italic" => pen.text(10.0, 10.5, 15.0, "I", c, false),
        "underline" => {
            pen.text(10.0, 9.0, 14.0, "U", c, false);
            pen.line(&[(5.0, 17.0), (15.0, 17.0)]);
        }
        "strike" => {
            pen.text(10.0, 10.0, 12.0, "ab", c, false);
            pen.line_c(&[(3.0, 10.5), (17.0, 10.5)], c);
        }
        "charborder" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, a);
            pen.text(10.0, 6.0, 9.0, "A", c, false);
        }
        "enclose" => {
            pen.circle(10.0, 10.5, 7.5, a);
            pen.text(10.0, 10.5, 9.0, "A", c, false);
        }
        "subscript" => {
            pen.text(8.0, 9.0, 13.0, "x", c, false);
            pen.text(15.0, 14.5, 8.0, "2", a, true);
        }
        "superscript" => {
            pen.text(8.0, 11.0, 13.0, "x", c, false);
            pen.text(15.0, 5.0, 8.0, "2", a, true);
        }
        "grow" => {
            pen.text(8.0, 11.0, 15.0, "A", c, false);
            pen.fill(&[(14.5, 6.0), (18.5, 6.0), (16.5, 3.0)], a);
        }
        "shrink" => {
            pen.text(8.0, 12.0, 11.0, "A", c, false);
            pen.fill(&[(14.5, 4.0), (18.5, 4.0), (16.5, 7.0)], a);
        }
        "case" => pen.text(10.0, 10.5, 12.0, "Aa", c, false),
        "clear" => {
            pen.text(8.0, 9.0, 13.0, "A", c, false);
            pen.fill(&[(10.0, 15.0), (14.0, 11.0), (18.0, 15.0), (14.0, 19.0)], Color32::from_rgb(0xE8, 0x6F, 0xA0));
        }
        // Small letters over a large one: text with a reading above it.
        "phonetic" => {
            pen.text(10.0, 5.0, 7.5, "abc", a, false);
            pen.text(10.0, 13.5, 12.5, "A", c, false);
        }
        "highlight" => {
            pen.fill(&[(5.0, 12.0), (11.0, 4.0), (14.0, 7.0), (8.0, 15.0)], c);
            pen.line(&[(5.0, 12.0), (3.5, 15.0), (8.0, 15.0)]);
            pen.frect(2.0, 16.5, 18.0, 19.0, yellow);
        }
        "fontcolor" => {
            pen.text(10.0, 9.0, 14.0, "A", c, false);
            pen.frect(2.0, 16.5, 18.0, 19.0, red);
        }
        "effects" => {
            pen.text(10.0, 10.0, 15.0, "A", a, true);
            pen.text(10.5, 10.5, 15.0, "A", Color32::from_rgba_unmultiplied(0, 0, 0, 40), true);
        }
        "fontdialog" => pen.line(&[(13.0, 13.0), (17.0, 17.0), (17.0, 13.0)]),
        // Paragraph
        "bullets" => {
            for (i, y) in [5.0, 10.0, 15.0].iter().enumerate() {
                pen.fcircle(4.0, *y, 1.4, if i == 0 { a } else { c });
                pen.line(&[(7.5, *y), (17.0, *y)]);
            }
        }
        "numbering" => {
            for (i, y) in [5.0, 10.0, 15.0].iter().enumerate() {
                pen.text(4.0, *y, 6.0, &(i + 1).to_string(), if i == 0 { a } else { c }, true);
                pen.line(&[(7.5, *y), (17.0, *y)]);
            }
        }
        "multilevel" => {
            pen.text(3.5, 4.5, 5.5, "1", a, true);
            pen.line(&[(6.5, 4.5), (17.0, 4.5)]);
            pen.text(6.5, 10.0, 5.5, "a", c, true);
            pen.line(&[(9.5, 10.0), (17.0, 10.0)]);
            pen.text(9.5, 15.5, 5.5, "i", c, true);
            pen.line(&[(12.0, 15.5), (17.0, 15.5)]);
        }
        "outdent" | "indent" => {
            pen.lines(9.0, 17.0, &[7.0, 10.0, 13.0]);
            pen.lines(3.0, 17.0, &[3.5, 16.5]);
            if name == "indent" {
                pen.fill(&[(3.0, 7.0), (3.0, 13.0), (6.5, 10.0)], a);
            } else {
                pen.fill(&[(6.5, 7.0), (6.5, 13.0), (3.0, 10.0)], a);
            }
        }
        "sort" => {
            pen.text(6.0, 5.5, 7.0, "A", c, true);
            pen.text(6.0, 14.5, 7.0, "Z", c, true);
            pen.line_c(&[(14.0, 3.0), (14.0, 17.0)], a);
            pen.line_c(&[(11.5, 14.5), (14.0, 17.0), (16.5, 14.5)], a);
        }
        "pilcrow" => pen.text(10.0, 10.5, 15.0, "¶", c, false),
        "alignLeft" => {
            pen.lines(3.0, 17.0, &[4.0, 10.0, 16.0]);
            ().then_lines(&pen, 3.0, 12.0, &[7.0, 13.0])
        }
        "alignCenter" => {
            pen.lines(3.0, 17.0, &[4.0, 10.0, 16.0]);
            ().then_lines(&pen, 6.0, 14.0, &[7.0, 13.0])
        }
        "alignRight" => {
            pen.lines(3.0, 17.0, &[4.0, 10.0, 16.0]);
            ().then_lines(&pen, 8.0, 17.0, &[7.0, 13.0])
        }
        "justify" => pen.lines(3.0, 17.0, &[4.0, 7.0, 10.0, 13.0, 16.0]),
        // Text direction: lines set from the start edge, an arrow in the reading direction.
        "textLtr" => {
            pen.lines(3.0, 14.0, &[4.0, 8.0]);
            pen.lines(3.0, 10.0, &[12.0]);
            pen.line_c(&[(3.0, 16.5), (16.5, 16.5)], a);
            pen.line_c(&[(13.5, 14.0), (16.5, 16.5), (13.5, 19.0)], a);
        }
        "textRtl" => {
            pen.lines(6.0, 17.0, &[4.0, 8.0]);
            pen.lines(10.0, 17.0, &[12.0]);
            pen.line_c(&[(17.0, 16.5), (3.5, 16.5)], a);
            pen.line_c(&[(6.5, 14.0), (3.5, 16.5), (6.5, 19.0)], a);
        }
        "lineSpacing" => {
            pen.lines(9.0, 17.0, &[5.0, 10.0, 15.0]);
            pen.line_c(&[(4.5, 3.0), (4.5, 17.0)], a);
            pen.line_c(&[(2.5, 5.0), (4.5, 3.0), (6.5, 5.0)], a);
            pen.line_c(&[(2.5, 15.0), (4.5, 17.0), (6.5, 15.0)], a);
        }
        "shading" => {
            pen.fill(&[(4.0, 9.0), (10.0, 3.0), (16.0, 9.0), (10.0, 15.0)], Color32::TRANSPARENT);
            pen.closed(&[(4.0, 9.0), (10.0, 3.0), (16.0, 9.0), (10.0, 15.0)], c);
            pen.fcircle(16.5, 13.5, 1.6, a);
            pen.frect(2.0, 16.5, 18.0, 19.0, yellow);
        }
        "borders" => {
            for y in [3.0, 10.0, 17.0] {
                for x in [3.0, 6.5, 10.0, 13.5, 17.0] {
                    pen.fcircle(x, y, 0.6, c);
                }
            }
            for x in [3.0, 10.0, 17.0] {
                for y in [6.5, 13.5] {
                    pen.fcircle(x, y, 0.6, c);
                }
            }
            pen.line_c(&[(3.0, 17.0), (17.0, 17.0)], a);
        }
        // Editing
        "find" => {
            pen.circle(8.0, 8.0, 5.0, c);
            pen.line_c(&[(11.5, 11.5), (17.0, 17.0)], a);
        }
        "replace" => {
            pen.text(6.0, 6.0, 8.0, "ab", c, false);
            pen.text(13.0, 15.0, 8.0, "ac", a, false);
            pen.line(&[(13.0, 3.5), (16.0, 3.5), (16.0, 8.5)]);
            pen.line(&[(14.5, 7.0), (16.0, 8.5), (17.5, 7.0)]);
        }
        "select" => {
            pen.fill(&[(5.0, 3.0), (5.0, 16.0), (8.0, 13.0), (10.5, 18.0), (12.5, 17.0), (10.0, 12.0), (14.0, 12.0)], c);
        }
        "dictate" => {
            pen.rect(7.5, 2.5, 12.5, 12.0, c);
            pen.line(&[(5.0, 9.5), (5.0, 11.0), (10.0, 15.0), (15.0, 11.0), (15.0, 9.5)]);
            pen.line(&[(10.0, 15.0), (10.0, 18.0)]);
        }
        "editor" | "spelling" => {
            pen.text(8.0, 7.0, 9.0, "abc", c, true);
            pen.line_c(&[(5.0, 13.0), (8.5, 16.5), (16.0, 9.0)], green);
        }
        "styles" => {
            pen.text(10.0, 10.0, 14.0, "A", a, false);
            pen.line(&[(13.0, 15.0), (17.0, 11.0)]);
        }
        "stylesPane" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.line(&[(11.0, 3.0), (11.0, 17.0)]);
            pen.text(7.0, 10.0, 8.0, "A", a, false);
        }
        // Insert
        "coverPage" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.frect(5.5, 7.0, 14.5, 10.0, a);
        }
        "blankPage" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.line_c(&[(10.0, 7.0), (10.0, 13.0)], a);
            pen.line_c(&[(7.0, 10.0), (13.0, 10.0)], a);
        }
        "pageBreak" => {
            pen.line(&[(4.0, 2.0), (4.0, 7.0), (16.0, 7.0), (16.0, 2.0)]);
            pen.line(&[(4.0, 18.0), (4.0, 13.0), (16.0, 13.0), (16.0, 18.0)]);
            pen.line_c(&[(2.0, 10.0), (18.0, 10.0)], a);
        }
        "drawTable" => {
            pen.rect(2.5, 3.0, 14.0, 14.5, c);
            pen.line(&[(2.5, 8.5), (14.0, 8.5)]);
            pen.line(&[(8.0, 3.0), (8.0, 14.5)]);
            pen.line(&[(9.5, 18.0), (17.0, 10.5), (18.5, 12.0), (11.0, 19.5), (9.0, 19.8), (9.5, 18.0)]);
            pen.fill(&[(17.0, 10.5), (18.0, 9.5), (19.5, 11.0), (18.5, 12.0)], a);
        }
        "table" => {
            pen.frect(3.0, 3.0, 17.0, 7.0, a);
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.line(&[(3.0, 7.0), (17.0, 7.0)]);
            pen.line(&[(3.0, 12.0), (17.0, 12.0)]);
            pen.line(&[(8.0, 3.0), (8.0, 17.0)]);
            pen.line(&[(12.5, 3.0), (12.5, 17.0)]);
        }
        "picture" => {
            pen.rect(2.5, 4.0, 17.5, 16.0, c);
            pen.fill(&[(4.0, 14.5), (8.5, 8.0), (12.0, 14.5)], a);
            pen.fill(&[(9.5, 14.5), (13.0, 10.0), (16.0, 14.5)], green);
            pen.fcircle(13.5, 7.0, 1.5, yellow);
        }
        "onlinePicture" => {
            paint(p, Rect::from_min_size(r.min, r.size() * 0.8), "picture", c, accent);
            pen.circle(15.0, 15.0, 3.5, a);
        }
        "shapes" => {
            pen.rect(3.0, 3.0, 11.0, 11.0, c);
            pen.fcircle(13.0, 13.0, 4.5, a);
        }
        // Shape Effects: a square casting a soft shadow down and to the right.
        "shapeEffects" => {
            for (k, alpha) in [(2.0, 30), (1.0, 50), (0.0, 80)] {
                pen.frect(7.0 - k, 7.0 - k, 18.0 + k, 18.0 + k, Color32::from_rgba_unmultiplied(0, 0, 0, alpha));
            }
            pen.frect(3.0, 3.0, 14.0, 14.0, a);
            pen.rect(3.0, 3.0, 14.0, 14.0, c);
        }
        "icons" => {
            pen.circle(10.0, 10.0, 7.5, c);
            pen.fcircle(7.5, 8.0, 1.0, c);
            pen.fcircle(12.5, 8.0, 1.0, c);
            pen.line_c(&[(6.5, 12.0), (10.0, 14.5), (13.5, 12.0)], a);
        }
        "models3d" => {
            pen.closed(&[(10.0, 2.5), (17.0, 6.0), (17.0, 14.0), (10.0, 17.5), (3.0, 14.0), (3.0, 6.0)], c);
            pen.line_c(&[(3.0, 6.0), (10.0, 9.5), (17.0, 6.0)], a);
            pen.line_c(&[(10.0, 9.5), (10.0, 17.5)], a);
        }
        "smartArt" => {
            pen.frect(7.0, 2.5, 13.0, 6.5, a);
            pen.rect(2.5, 13.0, 8.0, 17.0, c);
            pen.rect(12.0, 13.0, 17.5, 17.0, c);
            pen.line(&[(10.0, 6.5), (10.0, 10.0), (5.0, 10.0), (5.0, 13.0)]);
            pen.line(&[(10.0, 10.0), (15.0, 10.0), (15.0, 13.0)]);
        }
        "addShape" => {
            pen.frect(2.5, 3.0, 9.5, 8.0, a);
            pen.rect(2.5, 12.0, 9.5, 17.0, c);
            pen.line(&[(6.0, 8.0), (6.0, 12.0)]);
            pen.line_c(&[(14.5, 10.5), (14.5, 17.5)], green);
            pen.line_c(&[(11.0, 14.0), (18.0, 14.0)], green);
        }
        "promote" => {
            pen.lines(9.0, 17.5, &[5.0, 10.0, 15.0]);
            pen.line_c(&[(6.5, 7.5), (3.0, 10.0), (6.5, 12.5)], a);
        }
        "demote" => {
            pen.lines(9.0, 17.5, &[5.0, 10.0, 15.0]);
            pen.line_c(&[(3.0, 7.5), (6.5, 10.0), (3.0, 12.5)], a);
        }
        "textPane" => {
            pen.rect(2.5, 3.0, 17.5, 17.0, c);
            pen.line(&[(9.0, 3.0), (9.0, 17.0)]);
            pen.lines(4.0, 7.5, &[6.5, 10.0, 13.5]);
            pen.frect(11.0, 6.0, 16.0, 9.0, a);
            pen.frect(11.0, 11.5, 16.0, 14.5, a);
        }
        "smartArtColors" => {
            pen.fcircle(7.0, 7.5, 4.0, a);
            pen.fcircle(13.0, 7.5, 4.0, orange);
            pen.fcircle(10.0, 13.0, 4.0, green);
            pen.circle(10.0, 10.0, 8.0, c);
        }
        "resetGraphic" => {
            pen.rect(7.5, 6.5, 16.5, 15.5, c);
            pen.frect(9.5, 8.5, 14.5, 13.5, a);
            pen.line(&[(3.5, 13.0), (3.5, 4.0), (11.0, 4.0)]);
            pen.line(&[(1.5, 6.0), (3.5, 4.0), (5.5, 6.0)]);
        }
        "chart" => {
            pen.frect(3.5, 11.0, 6.5, 17.0, a);
            pen.frect(8.5, 6.0, 11.5, 17.0, orange);
            pen.frect(13.5, 9.0, 16.5, 17.0, green);
            pen.line(&[(2.0, 17.5), (18.0, 17.5)]);
        }
        "changeChartType" => {
            pen.frect(3.0, 11.0, 5.5, 17.0, a);
            pen.frect(7.0, 7.0, 9.5, 17.0, orange);
            pen.line(&[(2.0, 17.5), (11.0, 17.5)]);
            pen.fill(&[(15.0, 10.0), (15.0, 5.0), (19.0, 8.5)], green);
            pen.circle(15.0, 10.0, 4.0, c);
            pen.line_c(&[(4.0, 5.0), (9.0, 2.5), (12.0, 4.5)], a);
        }
        "chartData" => {
            pen.rect(2.5, 3.0, 17.5, 17.0, c);
            pen.frect(2.5, 3.0, 17.5, 6.5, a);
            pen.line(&[(2.5, 10.0), (17.5, 10.0)]);
            pen.line(&[(2.5, 13.5), (17.5, 13.5)]);
            pen.line(&[(7.5, 3.0), (7.5, 17.0)]);
            pen.line(&[(12.5, 3.0), (12.5, 17.0)]);
            pen.frect(9.0, 14.5, 11.0, 16.0, orange);
        }
        "chartTitle" => {
            pen.frect(5.0, 2.5, 15.0, 4.5, a);
            pen.frect(4.0, 11.0, 7.0, 17.0, c);
            pen.frect(8.5, 8.0, 11.5, 17.0, c);
            pen.frect(13.0, 12.5, 16.0, 17.0, c);
            pen.line(&[(2.5, 17.5), (17.5, 17.5)]);
        }
        "legend" => {
            for (k, col) in [a, orange, green].into_iter().enumerate() {
                let y = 5.0 + 5.0 * k as f32;
                pen.frect(3.0, y - 1.5, 6.0, y + 1.5, col);
                pen.line(&[(8.0, y), (17.0, y)]);
            }
        }
        "dataLabels" => {
            pen.frect(3.5, 11.0, 7.0, 17.5, a);
            pen.frect(12.0, 8.0, 15.5, 17.5, orange);
            pen.rect(2.5, 5.5, 8.0, 9.0, c);
            pen.rect(11.0, 2.5, 16.5, 6.0, c);
        }
        "screenshot" => {
            pen.line(&[(3.0, 7.0), (3.0, 3.0), (7.0, 3.0)]);
            pen.line(&[(13.0, 3.0), (17.0, 3.0), (17.0, 7.0)]);
            pen.line(&[(17.0, 13.0), (17.0, 17.0), (13.0, 17.0)]);
            pen.line(&[(7.0, 17.0), (3.0, 17.0), (3.0, 13.0)]);
            pen.fcircle(10.0, 10.0, 3.0, a);
        }
        "video" => {
            pen.rect(2.5, 5.0, 14.0, 15.0, c);
            pen.fill(&[(14.0, 10.0), (18.0, 6.5), (18.0, 13.5)], a);
        }
        "link" => {
            pen.closed(
                &[(3.0, 10.0), (7.0, 6.0), (9.5, 6.0), (9.5, 8.0), (7.5, 8.0), (5.0, 10.5), (7.5, 13.0), (9.5, 13.0), (9.5, 15.0), (7.0, 15.0)],
                c,
            );
            pen.line_c(&[(7.5, 10.5), (12.5, 10.5)], a);
            pen.closed(
                &[
                    (17.0, 10.5),
                    (13.0, 14.5),
                    (10.5, 14.5),
                    (10.5, 12.5),
                    (12.5, 12.5),
                    (15.0, 10.0),
                    (12.5, 7.5),
                    (10.5, 7.5),
                    (10.5, 5.5),
                    (13.0, 5.5),
                ],
                c,
            );
        }
        "bookmark" => {
            pen.closed(&[(5.5, 2.5), (14.5, 2.5), (14.5, 17.5), (10.0, 13.5), (5.5, 17.5)], c);
            pen.fill(&[(7.0, 4.0), (13.0, 4.0), (13.0, 8.0), (7.0, 8.0)], a);
        }
        "crossRef" => {
            pen.page(3.0, 2.0, 12.0, 14.0);
            pen.line_c(&[(9.0, 12.0), (17.0, 17.0)], a);
            pen.line_c(&[(14.0, 17.5), (17.0, 17.0), (16.5, 14.0)], a);
        }
        "comment" | "newComment" => {
            pen.closed(&[(2.5, 3.5), (17.5, 3.5), (17.5, 13.5), (9.0, 13.5), (5.0, 17.0), (5.0, 13.5), (2.5, 13.5)], c);
            if name == "newComment" {
                pen.line_c(&[(10.0, 6.0), (10.0, 11.0)], a);
                pen.line_c(&[(7.5, 8.5), (12.5, 8.5)], a);
            } else {
                pen.lines(5.5, 14.5, &[7.0, 10.0]);
            }
        }
        "header" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.frect(5.5, 5.0, 14.5, 7.5, a);
            pen.lines(6.0, 14.0, &[11.0, 13.5]);
        }
        "footer" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.lines(6.0, 14.0, &[6.5, 9.0]);
            pen.frect(5.5, 13.0, 14.5, 15.5, a);
        }
        "pageNumber" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.text(10.0, 13.0, 7.0, "#", a, true);
        }
        "textBox" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.text(10.0, 10.0, 10.0, "A", a, false);
        }
        "quickParts" => {
            pen.rect(3.0, 3.0, 9.0, 9.0, c);
            pen.frect(11.0, 3.0, 17.0, 9.0, a);
            pen.rect(3.0, 11.0, 9.0, 17.0, c);
            pen.rect(11.0, 11.0, 17.0, 17.0, c);
        }
        "wordArt" => {
            pen.text(10.0, 10.5, 15.0, "A", a, true);
            pen.line_c(&[(3.0, 17.5), (17.0, 17.5)], orange);
        }
        "textFill" => {
            pen.text(10.0, 8.5, 13.0, "A", c, true);
            pen.frect(3.0, 15.0, 17.0, 18.5, a);
        }
        "textOutline" => {
            pen.text(10.0, 8.5, 13.0, "A", a, false);
            pen.rect(3.0, 15.0, 17.0, 18.5, c);
        }
        "transform" => {
            // Letters along an arch.
            let arc: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let t = std::f32::consts::PI * (1.0 - i as f32 / 12.0);
                    (10.0 + 7.5 * t.cos(), 15.0 - 9.0 * t.sin())
                })
                .collect();
            pen.line_c(&arc, a);
            pen.text(10.0, 13.0, 8.0, "A", c, true);
            pen.line(&[(2.5, 17.5), (17.5, 17.5)]);
        }
        "dropCap" => {
            pen.text(6.0, 8.0, 12.0, "A", a, true);
            pen.lines(11.0, 17.0, &[4.0, 8.0, 12.0]);
            pen.lines(3.0, 17.0, &[16.0]);
        }
        "signature" => {
            pen.line_c(&[(3.0, 13.0), (6.0, 7.0), (8.0, 12.0), (11.0, 6.0), (13.0, 12.0)], a);
            pen.line(&[(3.0, 16.5), (17.0, 16.5)]);
        }
        "dateTime" => {
            pen.rect(3.0, 4.0, 17.0, 17.0, c);
            pen.frect(3.0, 4.0, 17.0, 7.5, a);
            pen.lines(5.0, 15.0, &[11.0, 14.0]);
        }
        "object" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.fcircle(10.0, 10.0, 3.5, a);
        }
        "equation" => pen.text(10.0, 10.0, 17.0, "π", a, false),
        "symbol" => pen.text(10.0, 10.0, 17.0, "Ω", c, false),
        // Draw
        "pen" => {
            pen.line(&[(4.0, 16.0), (14.0, 6.0), (16.0, 8.0), (6.0, 18.0), (3.5, 18.5), (4.0, 16.0)]);
            pen.fill(&[(14.0, 6.0), (15.5, 4.5), (17.5, 6.5), (16.0, 8.0)], a);
        }
        "pencil" => {
            pen.line(&[(4.0, 16.0), (14.0, 6.0), (16.0, 8.0), (6.0, 18.0), (3.5, 18.5), (4.0, 16.0)]);
            pen.fill(&[(14.0, 6.0), (15.5, 4.5), (17.5, 6.5), (16.0, 8.0)], orange);
        }
        "eraser" => {
            pen.closed(&[(3.0, 13.0), (11.0, 5.0), (17.0, 11.0), (11.0, 17.0), (7.0, 17.0)], c);
            pen.fill(&[(3.0, 13.0), (7.0, 9.0), (13.0, 15.0), (11.0, 17.0), (7.0, 17.0)], Color32::from_rgb(0xE8, 0x6F, 0xA0));
        }
        // Ink thickness: three strokes, thin to thick.
        "thickness" => {
            pen.frect(3.0, 4.5, 17.0, 5.5, c);
            pen.frect(3.0, 9.0, 17.0, 11.0, c);
            pen.frect(3.0, 14.0, 17.0, 17.0, a);
        }
        "lasso" => {
            pen.closed(&[(4.0, 7.0), (9.0, 3.0), (16.0, 5.0), (16.0, 11.0), (10.0, 13.0), (4.0, 11.0)], c);
            pen.line_c(&[(6.0, 12.5), (5.0, 17.5)], a);
        }
        "inkToShape" => {
            pen.line_c(&[(3.0, 10.0), (5.0, 6.0), (8.0, 12.0), (10.0, 7.0)], a);
            pen.rect(11.0, 9.0, 17.0, 15.0, c);
        }
        "inkToMath" => {
            pen.line_c(&[(3.0, 12.0), (5.0, 7.0), (7.0, 12.0)], a);
            pen.text(13.5, 10.0, 11.0, "√", c, false);
        }
        "canvas" => {
            pen.rect(2.5, 4.0, 17.5, 16.0, c);
            pen.line_c(&[(5.0, 13.0), (9.0, 8.0), (12.0, 11.0), (15.0, 7.0)], a);
        }
        "replay" => {
            pen.circle(10.0, 10.0, 6.5, c);
            pen.fill(&[(8.5, 7.0), (8.5, 13.0), (13.0, 10.0)], a);
        }
        // Add Pen: a pen with a plus.
        "addPen" => {
            pen.line(&[(3.0, 17.0), (11.0, 9.0), (13.0, 11.0), (5.0, 19.0), (2.5, 19.5), (3.0, 17.0)]);
            pen.fill(&[(11.0, 9.0), (12.5, 7.5), (14.5, 9.5), (13.0, 11.0)], a);
            pen.line_c(&[(15.0, 2.0), (15.0, 8.0)], a);
            pen.line_c(&[(12.0, 5.0), (18.0, 5.0)], a);
        }
        // Ink Replay's controls.
        "replayRewind" => {
            pen.frect(4.0, 5.0, 5.5, 15.0, c);
            pen.fill(&[(16.0, 5.0), (16.0, 15.0), (6.5, 10.0)], c);
        }
        "replayPlay" => pen.fill(&[(6.0, 4.0), (6.0, 16.0), (16.0, 10.0)], a),
        "replayPause" => {
            pen.frect(5.5, 4.5, 8.5, 15.5, a);
            pen.frect(11.5, 4.5, 14.5, 15.5, a);
        }
        "replayForward" => {
            pen.fill(&[(4.0, 5.0), (4.0, 15.0), (13.5, 10.0)], c);
            pen.frect(14.5, 5.0, 16.0, 15.0, c);
        }
        // Design
        "themes" => {
            pen.frect(3.0, 3.0, 10.0, 10.0, a);
            pen.frect(10.0, 3.0, 17.0, 10.0, orange);
            pen.frect(3.0, 10.0, 10.0, 17.0, green);
            pen.frect(10.0, 10.0, 17.0, 17.0, yellow);
        }
        "colors" => {
            pen.fcircle(7.0, 7.5, 3.5, red);
            pen.fcircle(13.0, 7.5, 3.5, a);
            pen.fcircle(10.0, 13.0, 3.5, green);
        }
        "fonts" => pen.text(10.0, 10.0, 13.0, "Aa", a, false),
        "paraSpacing" => {
            pen.lines(8.0, 17.0, &[4.0, 7.0, 13.0, 16.0]);
            pen.line_c(&[(4.0, 8.0), (4.0, 12.0)], a);
            pen.line_c(&[(2.5, 9.5), (4.0, 8.0), (5.5, 9.5)], a);
            pen.line_c(&[(2.5, 10.5), (4.0, 12.0), (5.5, 10.5)], a);
        }
        "effectsDesign" => {
            pen.fcircle(10.0, 10.0, 6.0, a.linear_multiply(0.5));
            pen.circle(10.0, 10.0, 6.0, c);
        }
        "setDefault" => {
            pen.line_c(&[(4.0, 10.5), (8.5, 15.0), (16.5, 5.0)], green);
        }
        "watermark" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.text(10.0, 11.0, 7.0, "W", a.linear_multiply(0.6), true);
        }
        "pageColor" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.frect(5.0, 9.0, 15.0, 17.0, a.linear_multiply(0.5));
        }
        "pageBorders" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.rect(6.0, 6.5, 14.0, 15.5, a);
        }
        // Layout
        "margins" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.frect(6.0, 4.5, 14.0, 15.5, a.linear_multiply(0.3));
        }
        "orientation" => {
            pen.page(3.0, 3.0, 11.0, 15.0);
            pen.rect(9.0, 10.0, 18.0, 17.0, a);
        }
        "size" => {
            pen.page(5.0, 2.0, 15.0, 18.0);
            pen.line_c(&[(2.5, 2.5), (2.5, 17.5)], a);
            pen.line_c(&[(5.0, 19.0), (15.0, 19.0)], a);
        }
        "columns" => {
            pen.lines(3.0, 9.0, &[4.0, 7.0, 10.0, 13.0, 16.0]);
            pen.lines(11.0, 17.0, &[4.0, 7.0, 10.0, 13.0]);
        }
        "breaks" => {
            pen.page(4.0, 1.5, 16.0, 8.5);
            pen.page(4.0, 11.5, 16.0, 18.5);
            pen.line_c(&[(2.0, 10.0), (18.0, 10.0)], a);
        }
        "lineNumbers" => {
            for (i, y) in [4.0, 8.0, 12.0, 16.0].iter().enumerate() {
                pen.text(4.0, *y, 5.0, &(i + 1).to_string(), a, true);
                pen.line(&[(7.0, *y), (17.0, *y)]);
            }
        }
        "hyphenation" => {
            pen.text(7.0, 7.0, 8.0, "ab-", c, false);
            pen.text(9.0, 14.5, 8.0, "cd", a, false);
        }
        "position" | "wrapText" => {
            pen.lines(3.0, 17.0, &[3.0, 17.0]);
            pen.lines(3.0, 6.0, &[7.0, 10.0, 13.0]);
            pen.frect(7.5, 6.0, 12.5, 14.0, a);
            pen.lines(14.0, 17.0, &[7.0, 10.0, 13.0]);
        }
        "bringForward" => {
            pen.rect(3.0, 3.0, 12.0, 12.0, c);
            pen.frect(8.0, 8.0, 17.0, 17.0, a);
        }
        "sendBackward" => {
            pen.frect(3.0, 3.0, 12.0, 12.0, a);
            pen.rect(8.0, 8.0, 17.0, 17.0, c);
        }
        "selectionPane" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.lines(6.0, 14.0, &[7.0, 10.0, 13.0]);
        }
        "align" => {
            pen.line_c(&[(3.0, 2.0), (3.0, 18.0)], a);
            pen.rect(4.5, 4.0, 15.0, 8.0, c);
            pen.rect(4.5, 11.0, 11.0, 15.0, c);
        }
        "group" => {
            pen.rect(2.5, 2.5, 17.5, 17.5, a);
            pen.rect(5.0, 5.0, 11.0, 11.0, c);
            pen.fcircle(13.0, 13.0, 3.0, c);
        }
        "rotate" => {
            pen.circle(10.0, 10.0, 6.0, c);
            pen.fill(&[(14.5, 3.0), (17.5, 6.0), (13.5, 7.0)], a);
        }
        "indentLeft" | "indentRight" | "spaceBefore" | "spaceAfter" => {
            pen.lines(3.0, 17.0, &[6.0, 10.0, 14.0]);
            pen.fcircle(if name.contains("Right") { 15.0 } else { 5.0 }, 10.0, 1.6, a);
        }
        // References
        "toc" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.lines(6.0, 11.0, &[7.0, 10.0, 13.0]);
            for y in [7.0, 10.0, 13.0] {
                pen.fcircle(13.5, y, 0.8, a);
            }
        }
        "addText" => {
            pen.lines(3.0, 13.0, &[5.0, 9.0, 13.0]);
            pen.line_c(&[(15.0, 11.0), (15.0, 17.0)], a);
            pen.line_c(&[(12.0, 14.0), (18.0, 14.0)], a);
        }
        "updateTable" | "update" => {
            pen.circle(10.0, 10.0, 6.0, c);
            pen.fill(&[(14.5, 3.0), (17.5, 6.0), (13.5, 7.0)], green);
        }
        "footnote" => {
            pen.text(6.5, 7.0, 10.0, "ab", c, false);
            pen.text(13.5, 4.5, 6.0, "1", a, true);
            pen.line(&[(3.0, 13.0), (9.0, 13.0)]);
            pen.lines(3.0, 17.0, &[16.0]);
        }
        "endnote" => {
            pen.text(6.5, 7.0, 10.0, "ab", c, false);
            pen.text(13.5, 4.5, 6.0, "i", a, true);
            pen.lines(3.0, 17.0, &[13.0, 16.0]);
        }
        "nextFootnote" => {
            pen.text(7.0, 7.0, 8.0, "ab", c, false);
            pen.text(13.0, 5.0, 6.0, "1", a, true);
            pen.line_c(&[(6.0, 15.0), (14.0, 15.0)], a);
            pen.line_c(&[(11.5, 12.5), (14.0, 15.0), (11.5, 17.5)], a);
        }
        "showNotes" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.frect(5.5, 13.0, 14.5, 16.5, a.linear_multiply(0.5));
        }
        "researcher" => {
            pen.circle(9.0, 9.0, 5.0, c);
            pen.line_c(&[(12.5, 12.5), (17.0, 17.0)], a);
            pen.lines(6.5, 11.5, &[8.0, 10.0]);
        }
        // Zotero
        "docPrefs" => {
            pen.page(3.0, 2.5, 13.0, 17.5);
            pen.lines(5.0, 10.0, &[7.5, 10.0]);
            // A small gear: ring and teeth.
            pen.circle(14.0, 14.0, 2.2, a);
            for k in 0..6 {
                let t = k as f32 * std::f32::consts::PI / 3.0;
                let (dx, dy) = (t.cos(), t.sin());
                pen.line_c(&[(14.0 + 2.8 * dx, 14.0 + 2.8 * dy), (14.0 + 4.2 * dx, 14.0 + 4.2 * dy)], a);
            }
        }
        "unlinkCitations" => {
            // Two chain links pulled apart, with a break between them.
            pen.closed(
                &[(2.5, 12.5), (6.0, 9.0), (8.5, 9.0), (8.5, 10.8), (6.8, 10.8), (4.6, 13.0), (6.8, 15.2), (8.5, 15.2), (8.5, 17.0), (6.0, 17.0)],
                c,
            );
            pen.closed(
                &[(17.5, 7.5), (14.0, 11.0), (11.5, 11.0), (11.5, 9.2), (13.2, 9.2), (15.4, 7.0), (13.2, 4.8), (11.5, 4.8), (11.5, 3.0), (14.0, 3.0)],
                c,
            );
            pen.line_c(&[(9.0, 6.5), (7.5, 5.0)], a);
            pen.line_c(&[(11.0, 13.5), (12.5, 15.0)], a);
            pen.line_c(&[(10.0, 5.5), (10.0, 3.5)], a);
            pen.line_c(&[(10.0, 14.5), (10.0, 16.5)], a);
        }
        "addNote" => {
            pen.closed(&[(3.0, 3.0), (17.0, 3.0), (17.0, 12.5), (12.5, 17.0), (3.0, 17.0)], c);
            pen.line(&[(12.5, 17.0), (12.5, 12.5), (17.0, 12.5)]);
            pen.line_c(&[(6.0, 8.0), (11.0, 8.0)], a);
            pen.line_c(&[(8.5, 5.5), (8.5, 10.5)], a);
            pen.lines(6.0, 11.0, &[13.5]);
        }
        "citation" => {
            pen.text(6.0, 9.0, 14.0, "“", a, true);
            pen.lines(9.0, 17.0, &[6.0, 10.0, 14.0]);
        }
        "sources" | "bibliography" => {
            pen.rect(4.0, 3.0, 9.0, 17.0, c);
            pen.rect(9.0, 3.0, 13.0, 17.0, a);
            pen.closed(&[(13.5, 4.0), (16.5, 3.5), (18.0, 16.5), (15.0, 17.0)], c);
        }
        "caption" => {
            pen.rect(3.0, 2.5, 17.0, 11.5, c);
            pen.fill(&[(4.5, 10.0), (8.0, 5.5), (11.0, 10.0)], a);
            pen.lines(3.0, 13.0, &[15.0, 17.5]);
        }
        "tableOfFigures" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.frect(6.0, 5.0, 10.0, 8.0, a);
            pen.lines(6.0, 14.0, &[11.0, 14.0]);
        }
        "markEntry" | "index" | "markCitation" | "tableOfAuthorities" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.text(10.0, 10.0, 8.0, if name == "index" || name == "markEntry" { "A" } else { "§" }, a, true);
        }
        // Mailings
        "envelope" => {
            pen.rect(2.5, 5.0, 17.5, 15.5, c);
            pen.line_c(&[(2.5, 5.0), (10.0, 11.0), (17.5, 5.0)], a);
        }
        "labels" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.frect(5.0, 5.0, 9.5, 9.0, a);
            pen.frect(10.5, 5.0, 15.0, 9.0, a);
            pen.frect(5.0, 11.0, 9.5, 15.0, a);
            pen.frect(10.5, 11.0, 15.0, 15.0, a);
        }
        "mailMerge" | "recipients" | "editRecipients" => {
            pen.fcircle(7.0, 7.0, 2.5, a);
            pen.line(&[(3.0, 15.0), (3.0, 13.0), (7.0, 10.5), (11.0, 13.0), (11.0, 15.0)]);
            pen.fcircle(13.5, 6.0, 2.0, c);
            pen.line(&[(11.5, 10.0), (13.5, 9.0), (17.0, 11.0), (17.0, 14.0)]);
        }
        "mergeField" | "addressBlock" | "greetingLine" | "rules" | "matchFields" | "highlightFields" => {
            pen.rect(2.5, 6.0, 17.5, 14.0, c);
            pen.text(10.0, 10.0, 7.0, "«»", a, true);
        }
        "preview" => {
            pen.text(10.0, 10.0, 8.0, "ABC", a, true);
            pen.rect(2.5, 5.0, 17.5, 15.0, c);
        }
        "next" | "previous" | "first" | "last" => {
            let pts: &[(f32, f32)] =
                if name == "next" || name == "last" { &[(7.0, 4.0), (13.0, 10.0), (7.0, 16.0)] } else { &[(13.0, 4.0), (7.0, 10.0), (13.0, 16.0)] };
            pen.line_c(pts, a);
        }
        "finish" => {
            pen.page(3.0, 2.0, 13.0, 15.0);
            pen.line_c(&[(10.0, 15.0), (13.0, 18.0), (18.0, 11.0)], green);
        }
        "checkErrors" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.fcircle(14.0, 14.0, 3.5, red);
        }
        // Review
        "thesaurus" => {
            pen.rect(3.5, 2.5, 16.5, 17.5, c);
            pen.text(10.0, 9.5, 8.0, "Aa", a, true);
        }
        "wordCount" => pen.text(10.0, 10.0, 9.0, "123", a, true),
        "readAloud" => {
            pen.text(6.0, 10.0, 10.0, "A", c, false);
            pen.line_c(&[(12.0, 7.0), (13.5, 10.0), (12.0, 13.0)], a);
            pen.line_c(&[(14.5, 5.0), (17.0, 10.0), (14.5, 15.0)], a);
        }
        "accessibility" => {
            pen.fcircle(10.0, 4.0, 1.6, a);
            pen.line(&[(4.0, 7.0), (16.0, 7.0)]);
            pen.line(&[(10.0, 7.0), (10.0, 12.0), (7.0, 17.5)]);
            pen.line(&[(10.0, 12.0), (13.0, 17.5)]);
        }
        "altText" => {
            // A small picture with lines of text beside and below it.
            pen.rect(2.5, 3.5, 11.5, 11.0, c);
            pen.line_c(&[(3.5, 10.0), (6.0, 6.5), (8.0, 8.5), (9.0, 7.5), (10.5, 10.0)], a);
            for (x0, x1, y) in [(13.5, 17.5, 5.0), (13.5, 17.5, 8.0), (2.5, 17.5, 14.0), (2.5, 13.0, 17.0)] {
                pen.line_c(&[(x0, y), (x1, y)], c);
            }
        }
        "translate" | "language" => {
            pen.text(6.5, 7.0, 9.0, "A", c, true);
            pen.text(13.5, 13.5, 9.0, "あ", a, false);
        }
        "deleteComment" => {
            pen.closed(&[(2.5, 3.5), (17.5, 3.5), (17.5, 13.5), (9.0, 13.5), (5.0, 17.0), (5.0, 13.5), (2.5, 13.5)], c);
            pen.line_c(&[(7.5, 6.0), (12.5, 11.0)], red);
            pen.line_c(&[(12.5, 6.0), (7.5, 11.0)], red);
        }
        "prevComment" | "nextComment" => {
            pen.closed(&[(2.5, 3.5), (17.5, 3.5), (17.5, 13.5), (9.0, 13.5), (5.0, 17.0), (5.0, 13.5), (2.5, 13.5)], c);
            let pts: &[(f32, f32)] =
                if name == "nextComment" { &[(8.0, 6.0), (12.0, 8.5), (8.0, 11.0)] } else { &[(12.0, 6.0), (8.0, 8.5), (12.0, 11.0)] };
            pen.line_c(pts, a);
        }
        "showComments" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.frect(12.0, 3.0, 17.0, 17.0, a.linear_multiply(0.4));
        }
        "resolve" => {
            pen.closed(&[(2.5, 3.5), (17.5, 3.5), (17.5, 13.5), (9.0, 13.5), (5.0, 17.0), (5.0, 13.5), (2.5, 13.5)], c);
            pen.line_c(&[(6.5, 8.5), (9.0, 11.0), (13.5, 6.0)], green);
        }
        "trackChanges" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.line_c(&[(6.0, 7.0), (14.0, 7.0)], red);
            pen.line_c(&[(6.0, 11.0), (14.0, 11.0)], a);
            pen.lines(6.0, 11.0, &[15.0]);
        }
        "markup" | "reviewingPane" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.line_c(&[(6.0, 8.0), (13.0, 8.0)], red);
            pen.lines(6.0, 13.0, &[12.0]);
        }
        "accept" => {
            pen.page(4.0, 2.0, 15.0, 17.0);
            pen.line_c(&[(10.0, 14.5), (12.5, 17.0), (18.0, 11.0)], green);
        }
        "reject" => {
            pen.page(4.0, 2.0, 15.0, 17.0);
            pen.line_c(&[(11.0, 12.0), (17.0, 18.0)], red);
            pen.line_c(&[(17.0, 12.0), (11.0, 18.0)], red);
        }
        "prevChange" | "nextChange" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            let pts: &[(f32, f32)] =
                if name == "nextChange" { &[(8.5, 7.0), (11.5, 10.0), (8.5, 13.0)] } else { &[(11.5, 7.0), (8.5, 10.0), (11.5, 13.0)] };
            pen.line_c(pts, a);
        }
        "compare" | "combine" => {
            pen.page(2.0, 3.0, 10.0, 15.0);
            pen.page(10.0, 5.0, 18.0, 17.0);
            pen.line_c(&[(6.0, 18.5), (14.0, 18.5)], a);
        }
        "protect" | "restrict" | "blockAuthors" => {
            pen.rect(5.0, 9.0, 15.0, 17.5, c);
            pen.line_c(&[(7.0, 9.0), (7.0, 6.0), (10.0, 3.0), (13.0, 6.0), (13.0, 9.0)], a);
        }
        "hideInk" => {
            pen.line_c(&[(3.0, 13.0), (6.0, 8.0), (9.0, 13.0), (12.0, 8.0)], a);
            pen.line(&[(3.0, 3.0), (17.0, 17.0)]);
        }
        // View
        "readMode" => {
            pen.closed(&[(2.0, 4.0), (9.5, 5.0), (9.5, 17.0), (2.0, 16.0)], c);
            pen.closed(&[(18.0, 4.0), (10.5, 5.0), (10.5, 17.0), (18.0, 16.0)], a);
        }
        "printLayout" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.lines(6.0, 14.0, &[7.0, 10.0, 13.0]);
        }
        "webLayout" => {
            pen.circle(10.0, 10.0, 7.0, c);
            pen.line_c(&[(3.0, 10.0), (17.0, 10.0)], a);
            pen.line_c(&[(10.0, 3.0), (7.0, 10.0), (10.0, 17.0)], a);
            pen.line_c(&[(10.0, 3.0), (13.0, 10.0), (10.0, 17.0)], a);
        }
        "outline" => {
            pen.fcircle(4.0, 4.5, 1.2, a);
            pen.line(&[(6.5, 4.5), (17.0, 4.5)]);
            pen.fcircle(7.0, 10.0, 1.2, a);
            pen.line(&[(9.5, 10.0), (17.0, 10.0)]);
            pen.fcircle(10.0, 15.5, 1.2, a);
            pen.line(&[(12.5, 15.5), (17.0, 15.5)]);
        }
        "draft" => pen.lines(3.0, 17.0, &[4.0, 7.0, 10.0, 13.0, 16.0]),
        "focus" => {
            pen.line(&[(3.0, 7.0), (3.0, 3.0), (7.0, 3.0)]);
            pen.line(&[(13.0, 3.0), (17.0, 3.0), (17.0, 7.0)]);
            pen.line(&[(17.0, 13.0), (17.0, 17.0), (13.0, 17.0)]);
            pen.line(&[(7.0, 17.0), (3.0, 17.0), (3.0, 13.0)]);
            pen.lines(7.0, 13.0, &[8.5, 11.5]);
        }
        "immersive" => {
            pen.closed(&[(2.0, 4.0), (9.5, 5.0), (9.5, 17.0), (2.0, 16.0)], c);
            pen.closed(&[(18.0, 4.0), (10.5, 5.0), (10.5, 17.0), (18.0, 16.0)], c);
            pen.fcircle(14.0, 10.0, 2.0, a);
        }
        "vertical" => {
            pen.page(5.0, 1.0, 15.0, 9.0);
            pen.page(5.0, 11.0, 15.0, 19.0);
        }
        "sideToSide" => {
            pen.page(1.5, 4.0, 9.5, 16.0);
            pen.page(10.5, 4.0, 18.5, 16.0);
        }
        "ruler" => {
            pen.rect(2.0, 7.0, 18.0, 13.0, c);
            for x in [4.5, 7.0, 9.5, 12.0, 14.5] {
                pen.line_c(&[(x, 7.0), (x, 10.0)], a);
            }
        }
        "gridlines" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            for k in [6.5, 10.0, 13.5] {
                pen.line_c(&[(k, 3.0), (k, 17.0)], a.linear_multiply(0.6));
                pen.line_c(&[(3.0, k), (17.0, k)], a.linear_multiply(0.6));
            }
        }
        "navPane" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.frect(3.0, 3.0, 8.0, 17.0, a.linear_multiply(0.5));
        }
        "zoom" => {
            pen.circle(8.5, 8.5, 5.5, c);
            pen.line_c(&[(12.5, 12.5), (17.5, 17.5)], a);
            pen.line(&[(6.0, 8.5), (11.0, 8.5)]);
            pen.line(&[(8.5, 6.0), (8.5, 11.0)]);
        }
        "zoomIn" | "zoomOut" => {
            pen.circle(8.5, 8.5, 5.5, c);
            pen.line_c(&[(12.5, 12.5), (17.5, 17.5)], a);
            pen.line(&[(6.0, 8.5), (11.0, 8.5)]);
            if name == "zoomIn" {
                pen.line(&[(8.5, 6.0), (8.5, 11.0)]);
            }
        }
        "zoom100" => pen.text(10.0, 10.0, 7.5, "100", a, true),
        "onePage" => pen.page(5.0, 2.0, 15.0, 18.0),
        "multiplePages" => {
            pen.page(1.5, 4.0, 9.5, 16.0);
            pen.page(10.5, 4.0, 18.5, 16.0);
        }
        "pageWidth" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.line_c(&[(1.5, 10.0), (18.5, 10.0)], a);
        }
        "darkMode" => {
            pen.fcircle(10.0, 10.0, 7.0, c);
            pen.fcircle(13.0, 8.0, 6.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230));
        }
        "interfaceTheme" => {
            // A window, its right half dark: light, dark or both (follow the system).
            pen.frect(10.0, 3.5, 17.5, 16.5, c);
            pen.rect(2.5, 3.5, 17.5, 16.5, c);
            pen.line_c(&[(2.5, 6.5), (10.0, 6.5)], a);
        }
        "newWindow" => {
            pen.rect(2.5, 5.0, 13.0, 15.0, c);
            pen.rect(7.0, 2.5, 17.5, 12.5, a);
        }
        "arrangeAll" => {
            pen.rect(3.0, 2.5, 17.0, 9.0, c);
            pen.rect(3.0, 11.0, 17.0, 17.5, a);
        }
        "split" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.line_c(&[(3.0, 10.0), (17.0, 10.0)], a);
        }
        "sideBySide" | "syncScroll" | "switchWindows" => {
            pen.rect(2.0, 4.0, 9.5, 16.0, c);
            pen.rect(10.5, 4.0, 18.0, 16.0, a);
        }
        "macros" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.fill(&[(8.0, 6.5), (8.0, 13.5), (14.0, 10.0)], a);
        }
        // A page with a filled band at the top and a style mark: the document's template.
        "documentTemplate" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.frect(6.0, 5.0, 11.5, 7.0, a);
            pen.text(10.0, 13.0, 8.0, "A", a, false);
        }
        "properties" | "info" => {
            pen.circle(10.0, 10.0, 7.0, c);
            pen.line_c(&[(10.0, 9.0), (10.0, 14.5)], a);
            pen.fcircle(10.0, 6.3, 1.0, a);
        }
        // Table tools
        "insertAbove" | "insertBelow" | "insertLeft" | "insertRight" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            match name {
                "insertAbove" => pen.frect(3.0, 3.0, 17.0, 8.0, a),
                "insertBelow" => pen.frect(3.0, 12.0, 17.0, 17.0, a),
                "insertLeft" => pen.frect(3.0, 3.0, 8.0, 17.0, a),
                _ => pen.frect(12.0, 3.0, 17.0, 17.0, a),
            }
        }
        "deleteTable" => {
            pen.rect(3.0, 3.0, 15.0, 15.0, c);
            pen.line(&[(3.0, 9.0), (15.0, 9.0)]);
            pen.line(&[(9.0, 3.0), (9.0, 15.0)]);
            pen.line_c(&[(12.0, 12.0), (18.0, 18.0)], red);
            pen.line_c(&[(18.0, 12.0), (12.0, 18.0)], red);
        }
        "merge" => {
            pen.rect(3.0, 5.0, 17.0, 15.0, c);
            pen.line_c(&[(5.0, 10.0), (15.0, 10.0)], a);
            pen.line_c(&[(7.0, 8.0), (5.0, 10.0), (7.0, 12.0)], a);
            pen.line_c(&[(13.0, 8.0), (15.0, 10.0), (13.0, 12.0)], a);
        }
        "splitCells" | "splitTable" => {
            pen.rect(3.0, 5.0, 17.0, 15.0, c);
            pen.line_c(&[(10.0, 5.0), (10.0, 15.0)], a);
        }
        "autofit" | "distributeRows" | "distributeCols" | "rowHeight" | "colWidth" => {
            pen.rect(3.0, 4.0, 17.0, 16.0, c);
            pen.line_c(&[(5.0, 10.0), (15.0, 10.0)], a);
            pen.line_c(&[(7.0, 8.0), (5.0, 10.0), (7.0, 12.0)], a);
            pen.line_c(&[(13.0, 8.0), (15.0, 10.0), (13.0, 12.0)], a);
        }
        "cellAlign" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.lines(6.0, 14.0, &[8.5, 11.5]);
        }
        "textDirection" => {
            pen.text(9.0, 10.0, 10.0, "A", c, false);
            pen.line_c(&[(15.5, 4.0), (15.5, 16.0)], a);
            pen.line_c(&[(13.5, 14.0), (15.5, 16.0), (17.5, 14.0)], a);
        }
        "cellMargins" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.rect(6.0, 6.0, 14.0, 14.0, a);
        }
        "repeatHeader" => {
            pen.frect(3.0, 3.0, 17.0, 7.0, a);
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.line(&[(3.0, 12.0), (17.0, 12.0)]);
        }
        "toText" => {
            pen.rect(3.0, 3.0, 11.0, 11.0, c);
            pen.lines(9.0, 17.0, &[14.0, 17.0]);
        }
        "formula" => pen.text(10.0, 10.0, 12.0, "fx", a, false),
        "selectTable" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.fill(&[(8.0, 7.0), (8.0, 15.0), (10.0, 13.0), (11.5, 16.0), (12.5, 15.5), (11.0, 12.5), (13.5, 12.5)], a);
        }
        "draw" => {
            pen.rect(3.0, 3.0, 13.0, 13.0, c);
            pen.line_c(&[(10.0, 17.0), (17.0, 10.0)], a);
        }
        "borderPainter" => {
            pen.line(&[(4.0, 16.0), (14.0, 6.0), (16.0, 8.0), (6.0, 18.0), (3.5, 18.5), (4.0, 16.0)]);
            pen.line_c(&[(3.0, 3.0), (11.0, 3.0)], a);
        }
        // Chrome
        "save" => {
            pen.closed(&[(3.0, 3.0), (14.0, 3.0), (17.0, 6.0), (17.0, 17.0), (3.0, 17.0)], c);
            pen.frect(6.0, 3.0, 13.0, 7.5, a);
            pen.rect(6.0, 11.0, 14.0, 17.0, c);
        }
        "undo" => {
            pen.line(&[(5.0, 8.0), (13.0, 8.0)]);
            pen.p.add(Shape::line(
                (0..=12)
                    .map(|i| {
                        let t = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
                        pen.pt(13.0 + 4.5 * t.cos(), 12.5 + 4.5 * t.sin())
                    })
                    .collect(),
                pen.stroke(c),
            ));
            pen.line(&[(13.0, 17.0), (8.0, 17.0)]);
            pen.fill(&[(2.5, 8.0), (6.5, 4.5), (6.5, 11.5)], a);
        }
        "redo" => {
            pen.line(&[(15.0, 8.0), (7.0, 8.0)]);
            pen.p.add(Shape::line(
                (0..=12)
                    .map(|i| {
                        let t = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
                        pen.pt(7.0 + 4.5 * t.cos(), 12.5 - 4.5 * t.sin())
                    })
                    .collect(),
                pen.stroke(c),
            ));
            pen.line(&[(7.0, 17.0), (12.0, 17.0)]);
            pen.fill(&[(17.5, 8.0), (13.5, 4.5), (13.5, 11.5)], a);
        }
        // Home screen (Margin handoff): a new page, a clock, sliders, grid and list views.
        "newDocument" => {
            pen.page(4.5, 2.5, 15.5, 17.5);
            pen.line(&[(10.0, 10.0), (10.0, 14.0)]);
            pen.line(&[(8.0, 12.0), (12.0, 12.0)]);
        }
        "recent" => {
            pen.circle(10.0, 10.0, 7.1, c);
            pen.line(&[(10.0, 6.2), (10.0, 10.0), (12.5, 11.7)]);
        }
        "settings" => {
            pen.lines(3.3, 10.8, &[5.8]);
            pen.lines(14.2, 16.7, &[5.8]);
            pen.lines(3.3, 5.8, &[14.2]);
            pen.lines(9.2, 16.7, &[14.2]);
            pen.circle(12.5, 5.8, 1.7, c);
            pen.circle(7.5, 14.2, 1.7, c);
        }
        "viewGrid" => {
            for (x, y) in [(3.5, 3.5), (11.5, 3.5), (3.5, 11.5), (11.5, 11.5)] {
                pen.rect(x, y, x + 5.0, y + 5.0, c);
            }
        }
        "viewList" => {
            pen.lines(7.0, 16.5, &[5.0, 10.0, 15.0]);
            pen.lines(3.5, 4.0, &[5.0, 10.0, 15.0]);
        }
        "search" => {
            pen.circle(8.5, 8.5, 5.0, c);
            pen.line(&[(12.0, 12.0), (16.5, 16.5)]);
        }
        "share" => {
            pen.line(&[(10.0, 3.0), (10.0, 12.0)]);
            pen.line(&[(6.5, 6.5), (10.0, 3.0), (13.5, 6.5)]);
            pen.line(&[(5.0, 9.0), (3.5, 9.0), (3.5, 17.0), (16.5, 17.0), (16.5, 9.0), (15.0, 9.0)]);
        }
        "user" => {
            pen.fcircle(10.0, 7.0, 3.5, a);
            pen.p.add(Shape::convex_polygon(
                (0..=16)
                    .map(|i| {
                        let t = std::f32::consts::PI * i as f32 / 16.0;
                        pen.pt(10.0 - 6.5 * t.cos(), 18.0 - 6.0 * t.sin())
                    })
                    .collect(),
                a,
                Stroke::NONE,
            ));
        }
        "close" => {
            pen.line(&[(5.0, 5.0), (15.0, 15.0)]);
            pen.line(&[(15.0, 5.0), (5.0, 15.0)]);
        }
        // A triangle filling most of its box: it is drawn at 9–12 pt beside combos and split
        // buttons, where a smaller one shrank to a dot (#535).
        "dropdown" => pen.fill(&[(3.0, 7.0), (17.0, 7.0), (10.0, 14.0)], c),
        "more" => {
            for x in [5.0, 10.0, 15.0] {
                pen.fcircle(x, 10.0, 1.3, c);
            }
        }
        "launcher" => {
            pen.line(&[(5.0, 5.0), (5.0, 15.0), (15.0, 15.0)]);
            pen.line(&[(8.0, 8.0), (15.0, 15.0)]);
            pen.line(&[(11.0, 15.0), (15.0, 15.0), (15.0, 11.0)]);
        }
        "pin" => {
            pen.line(&[(10.0, 12.0), (10.0, 18.0)]);
            pen.rect(7.0, 2.0, 13.0, 9.0, c);
            pen.line(&[(5.0, 12.0), (15.0, 12.0)]);
        }
        "plus" => {
            pen.line(&[(10.0, 4.0), (10.0, 16.0)]);
            pen.line(&[(4.0, 10.0), (16.0, 10.0)]);
        }
        "minus" => pen.line(&[(4.0, 10.0), (16.0, 10.0)]),
        // Margin's editor icons (design handoff section 11), drawn on the 20 grid.
        "inspector" => {
            pen.rect(3.0, 4.0, 17.0, 16.0, c);
            pen.line(&[(12.5, 4.0), (12.5, 16.0)]);
        }
        "review" => {
            pen.page(5.0, 2.0, 15.0, 18.0);
            pen.line(&[(7.5, 11.0), (9.5, 13.0), (13.0, 8.5)]);
        }
        "viewPages" => {
            pen.rect(5.0, 2.5, 15.0, 8.0, c);
            pen.rect(5.0, 12.0, 15.0, 17.5, c);
        }
        "viewPageless" => {
            pen.rect(5.0, 2.5, 15.0, 17.5, c);
            pen.lines(7.5, 12.5, &[7.0, 10.0, 13.0]);
        }
        "check" => pen.line_c(&[(4.0, 10.5), (8.5, 15.0), (16.5, 5.0)], c),
        "chevronLeft" => pen.line(&[(13.0, 4.0), (7.0, 10.0), (13.0, 16.0)]),
        "chevronRight" => pen.line(&[(7.0, 4.0), (13.0, 10.0), (7.0, 16.0)]),
        "chevronUp" => pen.line(&[(4.0, 13.0), (10.0, 7.0), (16.0, 13.0)]),
        "chevronDown" => pen.line(&[(4.0, 7.0), (10.0, 13.0), (16.0, 7.0)]),
        // Read Aloud player
        "mediaPlay" => pen.fill(&[(6.0, 4.0), (16.0, 10.0), (6.0, 16.0)], c),
        "mediaPause" => {
            pen.frect(5.5, 4.0, 8.5, 16.0, c);
            pen.frect(11.5, 4.0, 14.5, 16.0, c);
        }
        "mediaStop" => pen.frect(5.0, 5.0, 15.0, 15.0, c),
        "mediaPrev" => {
            pen.frect(4.0, 5.0, 6.0, 15.0, c);
            pen.fill(&[(16.0, 5.0), (7.0, 10.0), (16.0, 15.0)], c);
        }
        "mediaNext" => {
            pen.frect(14.0, 5.0, 16.0, 15.0, c);
            pen.fill(&[(4.0, 5.0), (13.0, 10.0), (4.0, 15.0)], c);
        }
        // Move past citation: a bracketed citation with an arrow hopping over it to the caret.
        "pastCitation" => {
            pen.line_c(&[(4.5, 9.0), (3.0, 9.0), (3.0, 17.0), (4.5, 17.0)], c);
            pen.line_c(&[(11.5, 9.0), (13.0, 9.0), (13.0, 17.0), (11.5, 17.0)], c);
            pen.lines(5.5, 10.5, &[12.0, 14.5]);
            pen.line_c(&[(4.0, 6.0), (8.0, 2.5), (14.0, 2.5), (16.5, 7.0)], a);
            pen.line_c(&[(14.0, 6.0), (16.5, 7.0), (17.5, 4.5)], a);
            pen.line_c(&[(17.0, 9.0), (17.0, 17.5)], a);
        }
        "chevronDoubleRight" => {
            pen.line(&[(4.0, 4.0), (9.0, 10.0), (4.0, 16.0)]);
            pen.line(&[(11.0, 4.0), (16.0, 10.0), (11.0, 16.0)]);
        }
        "addins" => {
            pen.rect(3.0, 3.0, 9.0, 9.0, c);
            pen.rect(11.0, 3.0, 17.0, 9.0, c);
            pen.rect(3.0, 11.0, 9.0, 17.0, c);
            pen.line_c(&[(14.0, 11.0), (14.0, 17.0)], a);
            pen.line_c(&[(11.0, 14.0), (17.0, 14.0)], a);
        }
        "discord" => {
            pen.fill(&[(3.0, 6.0), (7.0, 3.5), (13.0, 3.5), (17.0, 6.0), (17.5, 14.0), (13.5, 16.5), (6.5, 16.5), (2.5, 14.0)], a);
            pen.fcircle(7.5, 10.5, 1.5, Color32::WHITE);
            pen.fcircle(12.5, 10.5, 1.5, Color32::WHITE);
        }
        // Open: a folder with its flap lifted.
        "open" => {
            pen.closed(&[(2.5, 5.0), (7.5, 5.0), (9.0, 6.5), (15.5, 6.5), (15.5, 9.0)], c);
            pen.line(&[(2.5, 5.0), (2.5, 16.0)]);
            pen.closed(&[(2.5, 16.0), (5.0, 9.0), (18.0, 9.0), (15.5, 16.0)], a);
        }
        // Print: a printer with a sheet coming out.
        "print" => {
            pen.rect(6.0, 2.5, 14.0, 7.0, c);
            pen.rect(2.5, 7.0, 17.5, 14.0, c);
            pen.closed(&[(6.0, 11.5), (14.0, 11.5), (14.0, 17.5), (6.0, 17.5)], a);
            pen.fcircle(15.0, 9.5, 0.8, c);
        }
        "alignText" => {
            // A box with its text lines in the middle, arrows to the top and bottom edges.
            pen.rect(3.0, 2.5, 17.0, 17.5, c);
            pen.lines(5.5, 11.0, &[9.0, 11.5]);
            pen.line_c(&[(14.0, 5.0), (14.0, 15.0)], a);
            pen.line_c(&[(12.5, 6.5), (14.0, 5.0), (15.5, 6.5)], a);
            pen.line_c(&[(12.5, 13.5), (14.0, 15.0), (15.5, 13.5)], a);
        }
        "textBoxLink" | "textBoxUnlink" => {
            // Two boxes, the text running from the first into the second (broken: cut through).
            pen.rect(2.0, 3.0, 8.5, 11.0, c);
            pen.rect(11.5, 9.0, 18.0, 17.0, c);
            pen.lines(3.5, 7.0, &[5.5, 8.0]);
            pen.line_c(&[(5.0, 11.0), (5.0, 14.0), (11.5, 14.0)], a);
            pen.line_c(&[(9.5, 12.5), (11.5, 14.0), (9.5, 15.5)], a);
            if name == "textBoxUnlink" {
                pen.line_c(&[(6.5, 16.5), (9.0, 11.5)], c);
            }
        }

        // The built-in file browser (#576): a folder, a file and the home folder.
        "folder" => {
            pen.frect(2.5, 4.5, 8.5, 7.0, yellow);
            pen.frect(2.5, 6.5, 17.5, 16.0, yellow);
            pen.closed(&[(2.5, 4.5), (8.0, 4.5), (9.5, 6.5), (17.5, 6.5), (17.5, 16.0), (2.5, 16.0)], c);
        }
        "file" => {
            pen.page(4.5, 2.0, 15.5, 18.0);
            pen.lines(7.0, 13.0, &[9.0, 12.0, 15.0]);
        }
        "homeFolder" => {
            pen.closed(&[(3.0, 9.5), (10.0, 3.5), (17.0, 9.5)], c);
            pen.closed(&[(5.0, 8.5), (5.0, 17.0), (15.0, 17.0), (15.0, 8.5)], c);
            pen.frect(8.5, 12.0, 11.5, 17.0, a);
        }

        // Outlining tab: arrows moving a paragraph between outline levels and along the outline.
        "outlinePromote" | "outlineDemote" => {
            pen.lines(9.0, 17.0, &[6.0, 10.0, 14.0]);
            if name == "outlinePromote" {
                pen.line_c(&[(7.0, 10.0), (2.5, 10.0)], a);
                pen.line_c(&[(5.0, 7.5), (2.5, 10.0), (5.0, 12.5)], a);
            } else {
                pen.line_c(&[(2.5, 10.0), (7.0, 10.0)], a);
                pen.line_c(&[(4.5, 7.5), (7.0, 10.0), (4.5, 12.5)], a);
            }
        }
        "outlineTop" | "outlineBody" => {
            pen.lines(11.0, 17.0, &[6.0, 10.0, 14.0]);
            let (x, d) = if name == "outlineTop" { (3.0, 1.0) } else { (9.0, -1.0) };
            for k in [0.0, 3.5] {
                pen.line_c(&[(x + d * k + d * 2.5, 7.0), (x + d * k, 10.0), (x + d * k + d * 2.5, 13.0)], a);
            }
        }
        "outlineUp" | "outlineDown" => {
            pen.lines(3.0, 11.0, &[6.0, 10.0, 14.0]);
            let up = name == "outlineUp";
            let (tip, tail) = if up { (3.0, 17.0) } else { (17.0, 3.0) };
            pen.line_c(&[(15.0, tail), (15.0, tip)], a);
            let wing = if up { tip + 3.0 } else { tip - 3.0 };
            pen.line_c(&[(12.0, wing), (15.0, tip), (18.0, wing)], a);
        }
        "outlineExpand" | "outlineCollapse" => {
            pen.fcircle(10.0, 10.0, 6.5, c);
            pen.line_c(&[(6.5, 10.0), (13.5, 10.0)], Color32::WHITE);
            if name == "outlineExpand" {
                pen.line_c(&[(10.0, 6.5), (10.0, 13.5)], Color32::WHITE);
            }
        }
        "closeOutline" => {
            pen.page(4.0, 2.0, 16.0, 18.0);
            pen.line_c(&[(7.0, 8.0), (13.0, 14.0)], red);
            pen.line_c(&[(13.0, 8.0), (7.0, 14.0)], red);
        }
        "help" => {
            pen.circle(10.0, 10.0, 7.0, c);
            pen.text(10.0, 10.0, 10.0, "?", a, true);
        }
        // Developer › Controls: a field's frame with what it holds.
        "ccRichText" => {
            pen.rect(2.5, 5.0, 17.5, 15.0, a);
            pen.text(7.5, 10.0, 8.0, "A", c, true);
            pen.text(13.0, 10.5, 7.0, "a", c, false);
        }
        "ccPlainText" => {
            pen.rect(2.5, 5.0, 17.5, 15.0, a);
            pen.text(8.0, 10.0, 8.0, "A", c, false);
            pen.line(&[(12.0, 7.5), (12.0, 12.5)]);
        }
        "ccPicture" => {
            pen.rect(2.5, 4.0, 17.5, 16.0, a);
            pen.line(&[(4.5, 14.0), (8.5, 9.5), (11.5, 12.5), (13.5, 10.5), (15.5, 14.0)]);
            pen.fcircle(13.5, 7.0, 1.2, c);
        }
        "ccGallery" => {
            pen.rect(2.5, 3.0, 17.5, 17.0, a);
            pen.frect(5.0, 5.5, 9.0, 9.5, c);
            pen.frect(11.0, 5.5, 15.0, 9.5, c);
            pen.lines(5.0, 15.0, &[12.5, 15.0]);
        }
        "ccCheckBox" => {
            pen.rect(4.0, 4.0, 16.0, 16.0, c);
            pen.line_c(&[(6.5, 10.0), (9.0, 13.0), (14.0, 6.5)], a);
        }
        "ccComboBox" => {
            pen.rect(2.5, 6.0, 17.5, 14.0, c);
            pen.line(&[(13.0, 6.0), (13.0, 14.0)]);
            pen.lines(4.5, 10.5, &[10.0]);
            pen.arrow_down(15.25, 10.0, a);
        }
        "ccDropDown" => {
            pen.rect(2.5, 3.0, 17.5, 9.0, c);
            pen.line(&[(13.0, 3.0), (13.0, 9.0)]);
            pen.arrow_down(15.25, 6.0, a);
            pen.rect(2.5, 9.0, 13.0, 17.0, c);
            pen.lines(4.5, 11.0, &[11.5, 14.5]);
        }
        "ccDate" => {
            pen.rect(3.0, 4.5, 17.0, 17.0, c);
            pen.frect(3.0, 4.5, 17.0, 8.0, a);
            pen.line(&[(6.5, 3.0), (6.5, 6.0)]);
            pen.line(&[(13.5, 3.0), (13.5, 6.0)]);
            for (x, y) in [(6.5, 11.0), (10.0, 11.0), (13.5, 11.0), (6.5, 14.5), (10.0, 14.5)] {
                pen.fcircle(x, y, 0.9, c);
            }
        }
        "ccRepeat" => {
            pen.rect(2.5, 3.0, 17.5, 17.0, a);
            pen.rect(5.0, 5.5, 15.0, 9.5, c);
            pen.rect(5.0, 11.0, 15.0, 15.0, c);
            pen.line_c(&[(10.0, 15.0), (10.0, 18.5)], a);
        }
        "designMode" => {
            pen.line(&[(4.0, 16.0), (16.0, 16.0), (4.0, 4.0), (4.0, 16.0)]);
            pen.line_c(&[(4.0, 12.0), (8.0, 16.0)], a);
            pen.line(&[(7.0, 13.0), (7.0, 12.0)]);
        }
        _ => {
            // Fallback: a rounded tile with the first letter.
            p.rect_stroke(r.shrink(r.width() * 0.12), 3.0, Stroke::new(1.0, c), egui::StrokeKind::Middle);
            let ch = name.chars().next().map(|x| x.to_ascii_uppercase().to_string()).unwrap_or_default();
            p.text(r.center(), egui::Align2::CENTER_CENTER, ch, FontId::proportional(r.height() * 0.45), a);
        }
    }
}

trait ThenLines {
    fn then_lines(self, pen: &Pen, x0: f32, x1: f32, ys: &[f32]);
}

impl ThenLines for () {
    fn then_lines(self, pen: &Pen, x0: f32, x1: f32, ys: &[f32]) {
        pen.lines(x0, x1, ys);
    }
}

/// Draw an Insert › Icons library icon into `r` in `c`, from the same path data the inserted
/// picture is rendered from (`wordcraft_render::icon_lib`).
pub fn library(p: &Painter, r: Rect, icon: &wordcraft_render::icon_lib::Icon, c: Color32) {
    use wordcraft_render::icon_lib::{GRID, STROKE};
    let s = r.width().min(r.height()) / GRID;
    let o = r.center() - vec2(GRID / 2.0 * s, GRID / 2.0 * s);
    let stroke = Stroke::new(STROKE * s, c);
    for piece in icon.pieces() {
        let pts: Vec<Pos2> = piece.points.iter().map(|q| pos2(o.x + q[0] * s, o.y + q[1] * s)).collect();
        if piece.fill {
            p.add(Shape::convex_polygon(pts, c, Stroke::NONE));
        } else if piece.closed {
            p.add(Shape::closed_line(pts, stroke));
        } else {
            // Round caps, as on the inserted picture.
            for end in [pts.first(), pts.last()].into_iter().flatten() {
                p.circle_filled(*end, stroke.width / 2.0, c);
            }
            p.add(Shape::line(pts, stroke));
        }
    }
}

/// The Margin icon set from the design handoff (`docs/design-handoff/claude-design/icons`):
/// 24 × 24 grid, stroke 1.5, round caps and joins, one colour. The geometry below is the SVG
/// data verbatim (a test compares it with the SVG files); a small SVG path interpreter turns it
/// into egui shapes.
mod handoff {
    use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

    const GRID: f32 = 24.0;
    const STROKE: f32 = 1.5;

    #[derive(Clone, Copy)]
    pub(super) enum Item {
        /// `<path d>` stroked.
        Path(&'static str),
        /// `<path d stroke-dasharray="dash gap">`.
        Dashed(&'static str, f32, f32),
        /// `<rect x y width height rx>` stroked.
        Rect(f32, f32, f32, f32, f32),
        /// `<circle cx cy r>` stroked.
        Circle(f32, f32, f32),
        /// `<circle cx cy r>` filled.
        Dot(f32, f32, f32),
    }
    use Item::{Circle, Dashed, Dot, Path, Rect as R};

    /// Handoff icon for a name call sites use (camelCase, as in the rest of this file, plus the
    /// handoff's own names), or `None` when the name belongs to the older set.
    pub(super) fn shapes(name: &str) -> Option<&'static [Item]> {
        Some(match name {
            "accept" => &[Path("M5 12.5l4.5 4.5L19 7.5")],
            "alignCenter" => &[Path("M4 6h16M7 10h10M4 14h16M7 18h10")],
            "justify" => &[Path("M4 6h16M4 10h16M4 14h16M4 18h16")],
            "alignLeft" => &[Path("M4 6h16M4 10h10M4 14h16M4 18h10")],
            "alignRight" => &[Path("M4 6h16M10 10h10M4 14h16M10 18h10")],
            "assistant" => &[Path("M8 4.5v15"), R(8.0, 4.5, 9.5, 5.0, 1.5)],
            "chevronDown" => &[Path("M7 10l5 5 5-5")],
            "chevronRight" => &[Path("M10 7l5 5-5 5")],
            "chevronUp" => &[Path("M7 14l5-5 5 5")],
            "comment" | "newComment" => &[Path("M4.5 6.5a2 2 0 0 1 2-2h11a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H11l-4.5 3.5v-3.5a2 2 0 0 1-2-2z")],
            "share" | "export" => &[Path("M12 15V4M8 8l4-4 4 4"), Path("M5 12.5V18a1.5 1.5 0 0 0 1.5 1.5h11A1.5 1.5 0 0 0 19 18v-5.5")],
            "focus" => &[Path(
                "M4 9V5.5A1.5 1.5 0 0 1 5.5 4H9M15 4h3.5A1.5 1.5 0 0 1 20 5.5V9M20 15v3.5a1.5 1.5 0 0 1-1.5 1.5H15M9 20H5.5A1.5 1.5 0 0 1 4 18.5V15",
            )],
            "picture" | "image" => &[R(3.5, 4.5, 17.0, 15.0, 2.0), Circle(9.0, 9.5, 1.5), Path("M4 17l5-4.5 3.5 3 3-2.5 4.5 4")],
            "inspector" => &[R(3.5, 4.5, 17.0, 15.0, 2.0), Path("M15 4.5v15")],
            "link" => &[Path("M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1"), Path("M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1")],
            "lock" => &[R(5.5, 10.5, 13.0, 9.0, 2.0), Path("M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5")],
            "more" => &[Dot(6.0, 12.0, 1.4), Dot(12.0, 12.0, 1.4), Dot(18.0, 12.0, 1.4)],
            "newDocument" => &[
                Path("M13.5 3.5H7a1.5 1.5 0 0 0-1.5 1.5v14A1.5 1.5 0 0 0 7 20.5h10a1.5 1.5 0 0 0 1.5-1.5V8.5z"),
                Path("M13.5 3.5v5h5"),
                Path("M12 11.5v5M9.5 14h5"),
            ],
            "open" => &[Path("M3.5 7a1.5 1.5 0 0 1 1.5-1.5h4l2 2h8a1.5 1.5 0 0 1 1.5 1.5v8.5a1.5 1.5 0 0 1-1.5 1.5H5A1.5 1.5 0 0 1 3.5 17z")],
            "popup" => &[Path("M8 9l4-4 4 4M8 15l4 4 4-4")],
            "print" => &[
                Path("M7 9V4.5h10V9"),
                Path("M7 16.5H5.5A1.5 1.5 0 0 1 4 15v-4.5A1.5 1.5 0 0 1 5.5 9h13a1.5 1.5 0 0 1 1.5 1.5V15a1.5 1.5 0 0 1-1.5 1.5H17"),
                R(7.0, 13.5, 10.0, 6.0, 1.0),
            ],
            "recent" => &[Circle(12.0, 12.0, 8.5), Path("M12 7.5V12l3 2")],
            "reject" => &[Path("M7 7l10 10M17 7L7 17")],
            "resolve" => &[Circle(12.0, 12.0, 8.5), Path("M8.5 12.5l2.5 2.5 4.5-5")],
            "review" => &[Path("M6.5 3.5h7l4 4v13h-11z"), Path("M9 13.5l2 2 4-4")],
            "saveAs" => &[Path("M12 4v10M8 10l4 4 4-4M5 19.5h14")],
            "search" => &[Circle(11.0, 11.0, 6.0), Path("M15.5 15.5L20 20")],
            "send" => &[Path("M12 19V5M6 11l6-6 6 6")],
            "settings" => &[Path("M4 7h9M17 7h3M4 17h3M11 17h9"), Circle(15.0, 7.0, 2.0), Circle(9.0, 17.0, 2.0)],
            "shapes" | "shape" => &[Circle(9.0, 9.0, 5.0), R(10.5, 10.5, 9.0, 9.0, 1.5)],
            "sidebarLeft" => &[R(3.5, 4.5, 17.0, 15.0, 2.0), Path("M9 4.5v15")],
            "stop" => &[R(7.0, 7.0, 10.0, 10.0, 2.0)],
            "table" => &[R(3.5, 4.5, 17.0, 15.0, 2.0), Path("M3.5 9.5h17M3.5 14.5h17M9.5 9.5v10M15 9.5v10")],
            "trackChanges" => &[Path("M4 20h4L18.5 9.5a2.12 2.12 0 0 0-3-3L5 17v3z"), Path("M13.5 8.5l3 3"), Dashed("M13 20h7", 2.0, 2.5)],
            "viewGrid" => &[R(4.0, 4.0, 6.5, 6.5, 1.0), R(13.5, 4.0, 6.5, 6.5, 1.0), R(4.0, 13.5, 6.5, 6.5, 1.0), R(13.5, 13.5, 6.5, 6.5, 1.0)],
            "viewList" => &[Path("M8.5 6.5h11.5M8.5 12h11.5M8.5 17.5h11.5M4 6.5h.5M4 12h.5M4 17.5h.5")],
            "viewPageless" => &[R(5.5, 3.0, 13.0, 18.0, 1.0), Path("M8.5 8h7M8.5 11.5h7M8.5 15h5")],
            "viewPages" => &[R(5.5, 3.0, 13.0, 8.5, 1.0), R(5.5, 13.5, 13.0, 8.0, 1.0)],
            _ => return None,
        })
    }

    /// A polyline in grid units; `closed` for `z` and rectangles.
    struct Poly {
        pts: Vec<(f32, f32)>,
        closed: bool,
    }

    struct Num<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl Num<'_> {
        fn skip(&mut self) {
            while self.b.get(self.i).is_some_and(|c| c.is_ascii_whitespace() || *c == b',') {
                self.i += 1;
            }
        }
        /// The next command letter, if one is next.
        fn cmd(&mut self) -> Option<u8> {
            self.skip();
            let c = *self.b.get(self.i)?;
            if c.is_ascii_alphabetic() {
                self.i += 1;
                Some(c)
            } else {
                None
            }
        }
        fn has_num(&mut self) -> bool {
            self.skip();
            self.b.get(self.i).is_some_and(|c| c.is_ascii_digit() || matches!(*c, b'-' | b'+' | b'.'))
        }
        fn num(&mut self) -> Option<f32> {
            self.skip();
            let start = self.i;
            if matches!(self.b.get(self.i), Some(b'-' | b'+')) {
                self.i += 1;
            }
            let mut dot = false;
            while let Some(&c) = self.b.get(self.i) {
                if c.is_ascii_digit() {
                    self.i += 1;
                } else if c == b'.' && !dot {
                    dot = true;
                    self.i += 1;
                } else {
                    break;
                }
            }
            std::str::from_utf8(self.b.get(start..self.i)?).ok()?.parse().ok()
        }
    }

    /// SVG elliptical arc from `p0` to `p1` (endpoint form), appended as points (after `p0`).
    #[allow(clippy::too_many_arguments)]
    fn arc(out: &mut Vec<(f32, f32)>, p0: (f32, f32), rx: f32, ry: f32, rot_deg: f32, large: bool, sweep: bool, p1: (f32, f32)) {
        if (p0.0 - p1.0).abs() < 1e-6 && (p0.1 - p1.1).abs() < 1e-6 {
            return;
        }
        let (mut rx, mut ry) = (rx.abs(), ry.abs());
        if rx < 1e-6 || ry < 1e-6 {
            out.push(p1);
            return;
        }
        let phi = rot_deg.to_radians();
        let (sp, cp) = phi.sin_cos();
        let dx = (p0.0 - p1.0) / 2.0;
        let dy = (p0.1 - p1.1) / 2.0;
        let x1 = cp * dx + sp * dy;
        let y1 = -sp * dx + cp * dy;
        let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        if lam > 1.0 {
            let k = lam.sqrt();
            rx *= k;
            ry *= k;
        }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let mut co = if den.abs() < 1e-9 { 0.0 } else { (num / den).max(0.0).sqrt() };
        if large == sweep {
            co = -co;
        }
        let cxp = co * rx * y1 / ry;
        let cyp = -co * ry * x1 / rx;
        let cx = cp * cxp - sp * cyp + (p0.0 + p1.0) / 2.0;
        let cy = sp * cxp + cp * cyp + (p0.1 + p1.1) / 2.0;
        let ang = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
        let th1 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
        let mut dth = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
        if !sweep && dth > 0.0 {
            dth -= std::f32::consts::TAU;
        } else if sweep && dth < 0.0 {
            dth += std::f32::consts::TAU;
        }
        let n = ((dth.abs() / (std::f32::consts::PI / 16.0)).ceil() as usize).max(2);
        for k in 1..=n {
            let t = th1 + dth * (k as f32 / n as f32);
            let (st, ct) = t.sin_cos();
            out.push((cp * rx * ct - sp * ry * st + cx, sp * rx * ct + cp * ry * st + cy));
        }
        if let Some(last) = out.last_mut() {
            *last = p1;
        }
    }

    /// Flatten SVG path data (`M L H V A Z`, absolute and relative) into polylines.
    fn flatten(d: &str) -> Vec<Poly> {
        let mut polys: Vec<Poly> = Vec::new();
        let mut cur = (0.0f32, 0.0f32);
        let mut start = cur;
        let mut t = Num { b: d.as_bytes(), i: 0 };
        let mut cmd = b'M';
        while t.i < t.b.len() {
            if let Some(c) = t.cmd() {
                cmd = c;
                if matches!(cmd, b'z' | b'Z') {
                    if let Some(p) = polys.last_mut() {
                        p.closed = true;
                    }
                    cur = start;
                    continue;
                }
            } else if !t.has_num() {
                break;
            }
            let rel = cmd.is_ascii_lowercase();
            let (ox, oy) = if rel { cur } else { (0.0, 0.0) };
            match cmd.to_ascii_uppercase() {
                b'M' | b'L' => {
                    let (Some(x), Some(y)) = (t.num(), t.num()) else { break };
                    cur = (ox + x, oy + y);
                    if cmd.eq_ignore_ascii_case(&b'M') {
                        start = cur;
                        polys.push(Poly { pts: vec![cur], closed: false });
                        cmd = if rel { b'l' } else { b'L' };
                    } else if let Some(p) = polys.last_mut() {
                        p.pts.push(cur);
                    }
                }
                b'H' => {
                    let Some(x) = t.num() else { break };
                    cur.0 = ox + x;
                    if let Some(p) = polys.last_mut() {
                        p.pts.push(cur);
                    }
                }
                b'V' => {
                    let Some(y) = t.num() else { break };
                    cur.1 = oy + y;
                    if let Some(p) = polys.last_mut() {
                        p.pts.push(cur);
                    }
                }
                b'A' => {
                    let (Some(rx), Some(ry), Some(rot), Some(la), Some(sw), Some(x), Some(y)) =
                        (t.num(), t.num(), t.num(), t.num(), t.num(), t.num(), t.num())
                    else {
                        break;
                    };
                    let end = (ox + x, oy + y);
                    if let Some(p) = polys.last_mut() {
                        arc(&mut p.pts, cur, rx, ry, rot, la != 0.0, sw != 0.0, end);
                    }
                    cur = end;
                }
                _ => break,
            }
        }
        polys
    }

    fn rounded_rect(x: f32, y: f32, w: f32, h: f32, rx: f32) -> Poly {
        let r = rx.min(w / 2.0).min(h / 2.0);
        let mut pts = Vec::new();
        let corners = [(x + w - r, y + r, -90.0f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)];
        for (cx, cy, a0) in corners {
            let n = if r > 0.0 { 6 } else { 0 };
            if n == 0 {
                pts.push((cx, cy));
                continue;
            }
            for k in 0..=n {
                let a = (a0 + 90.0 * k as f32 / n as f32).to_radians();
                pts.push((cx + r * a.cos(), cy + r * a.sin()));
            }
        }
        if r == 0.0 {
            pts = vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        }
        Poly { pts, closed: true }
    }

    /// Cut a polyline into dashes (`on`/`off` in grid units).
    fn dashes(pts: &[(f32, f32)], on: f32, off: f32) -> Vec<Vec<(f32, f32)>> {
        let mut out = Vec::new();
        let mut cur: Vec<(f32, f32)> = Vec::new();
        let (mut drawing, mut left) = (true, on);
        if let Some(&first) = pts.first() {
            cur.push(first);
        }
        for w in pts.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
            let mut pos = 0.0;
            while len - pos > left && left > 0.0 {
                pos += left;
                let f = pos / len;
                let q = (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f);
                if drawing {
                    cur.push(q);
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur = vec![q];
                }
                drawing = !drawing;
                left = if drawing { on } else { off };
            }
            left -= len - pos;
            if drawing {
                cur.push((x1, y1));
            }
        }
        if drawing && cur.len() > 1 {
            out.push(cur);
        }
        out
    }

    pub(super) fn paint(p: &Painter, r: Rect, items: &[Item], c: Color32) {
        let s = r.width().min(r.height()) / GRID;
        let o = r.center() - vec2(GRID / 2.0 * s, GRID / 2.0 * s);
        let at = |q: (f32, f32)| -> Pos2 { pos2(o.x + q.0 * s, o.y + q.1 * s) };
        let stroke = Stroke::new(STROKE * s, c);
        let cap = |q: Pos2| {
            p.circle_filled(q, stroke.width / 2.0, c);
        };
        let line = |pts: &[(f32, f32)], closed: bool| {
            let v: Vec<Pos2> = pts.iter().map(|&q| at(q)).collect();
            if closed {
                p.add(Shape::closed_line(v, stroke));
            } else {
                // Round caps, as in the SVG.
                if let Some(&q) = v.first() {
                    cap(q);
                }
                if let Some(&q) = v.last() {
                    cap(q);
                }
                p.add(Shape::line(v, stroke));
            }
        };
        for item in items {
            match *item {
                Item::Path(d) => {
                    for poly in flatten(d) {
                        line(&poly.pts, poly.closed);
                    }
                }
                Item::Dashed(d, on, off) => {
                    for poly in flatten(d) {
                        for seg in dashes(&poly.pts, on, off) {
                            line(&seg, false);
                        }
                    }
                }
                Item::Rect(x, y, w, h, rx) => {
                    let poly = rounded_rect(x, y, w, h, rx);
                    line(&poly.pts, true);
                }
                Item::Circle(cx, cy, rad) => {
                    p.circle_stroke(at((cx, cy)), rad * s, stroke);
                }
                Item::Dot(cx, cy, rad) => {
                    p.circle_filled(at((cx, cy)), rad * s, c);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Canonical text for one SVG element / one table item.
        fn canon(item: &Item) -> String {
            match *item {
                Item::Path(d) => format!("P {d}"),
                Item::Dashed(d, a, b) => format!("D {d} {a} {b}"),
                Item::Rect(x, y, w, h, rx) => format!("R {x} {y} {w} {h} {rx}"),
                Item::Circle(x, y, r) => format!("C {x} {y} {r}"),
                Item::Dot(x, y, r) => format!("F {x} {y} {r}"),
            }
        }

        fn attr(el: &str, name: &str) -> Option<String> {
            let key = format!(" {name}=\"");
            let i = el.find(&key)? + key.len();
            let j = el[i..].find('"')?;
            Some(el[i..i + j].to_string())
        }

        fn svg_items(svg: &str) -> Vec<String> {
            let filled = svg.contains("fill=\"currentColor\"");
            let mut out = Vec::new();
            for el in svg.split('<').skip(2).filter(|e| !e.starts_with('/')) {
                let f = |n: &str| attr(el, n).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                if el.starts_with("path") {
                    let d = attr(el, "d").unwrap_or_default();
                    match attr(el, "stroke-dasharray") {
                        Some(a) => out.push(format!(
                            "D {d} {}",
                            a.split_whitespace().map(|v| v.parse::<f32>().unwrap_or(0.0).to_string()).collect::<Vec<_>>().join(" ")
                        )),
                        None => out.push(format!("P {d}")),
                    }
                } else if el.starts_with("rect") {
                    out.push(format!("R {} {} {} {} {}", f("x"), f("y"), f("width"), f("height"), f("rx")));
                } else if el.starts_with("circle") {
                    out.push(format!("{} {} {} {}", if filled { "F" } else { "C" }, f("cx"), f("cy"), f("r")));
                }
            }
            out
        }

        /// (svg file stem, icon name used in code)
        const NAMES: [(&str, &str); 38] = [
            ("accept", "accept"),
            ("align_center", "alignCenter"),
            ("align_justify", "justify"),
            ("align_left", "alignLeft"),
            ("align_right", "alignRight"),
            ("assistant", "assistant"),
            ("chevron_down", "chevronDown"),
            ("chevron_right", "chevronRight"),
            ("chevron_up", "chevronUp"),
            ("comment", "comment"),
            ("export", "export"),
            ("focus", "focus"),
            ("image", "image"),
            ("inspector", "inspector"),
            ("link", "link"),
            ("lock", "lock"),
            ("more", "more"),
            ("new_document", "newDocument"),
            ("open", "open"),
            ("popup", "popup"),
            ("print", "print"),
            ("recent", "recent"),
            ("reject", "reject"),
            ("resolve", "resolve"),
            ("review", "review"),
            ("save_as", "saveAs"),
            ("search", "search"),
            ("send", "send"),
            ("settings", "settings"),
            ("shape", "shape"),
            ("sidebar_left", "sidebarLeft"),
            ("stop", "stop"),
            ("table", "table"),
            ("track_changes", "trackChanges"),
            ("view_grid", "viewGrid"),
            ("view_list", "viewList"),
            ("view_pageless", "viewPageless"),
            ("view_pages", "viewPages"),
        ];

        #[test]
        fn geometry_matches_the_handoff_svgs() {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/design-handoff/claude-design/icons");
            for (stem, name) in NAMES {
                let svg = std::fs::read_to_string(dir.join(format!("{stem}.svg"))).unwrap();
                let want = svg_items(&svg);
                let got: Vec<String> = shapes(name).unwrap().iter().map(canon).collect();
                assert_eq!(got, want, "{stem} ({name})");
            }
        }

        #[test]
        fn path_flattening_hits_the_endpoints() {
            // Quarter arc ends exactly where the data says.
            let polys = flatten("M4.5 6.5a2 2 0 0 1 2-2h11");
            assert_eq!(polys.len(), 1);
            let pts = &polys[0].pts;
            assert!(pts.contains(&(6.5, 4.5)));
            assert_eq!(pts.last().copied(), Some((17.5, 4.5)));
            // Compact numbers: "-" and "." separate values; "z" closes.
            let v = flatten("M4 6.5h.5M4 12h.5");
            assert_eq!(v.len(), 2);
            assert_eq!(v[0].pts, vec![(4.0, 6.5), (4.5, 6.5)]);
            let z = flatten("M6.5 3.5h7l4 4v13h-11z");
            assert!(z[0].closed);
            assert_eq!(z[0].pts.len(), 5);
            // Dashes: 7 units at 2 on / 2.5 off give 2 dashes (0-2, 4.5-6.5).
            assert_eq!(dashes(&[(0.0, 0.0), (7.0, 0.0)], 2.0, 2.5).len(), 2);
        }
    }
}
