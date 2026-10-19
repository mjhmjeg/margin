//! Optimo's Review panel (HANDOFF §7, R2–R9): the right-hand panel that replaces the Comments
//! pane in the Mac layout. A thin view over the review commands: tracking switch and markup
//! display, people with their change and comment counts, one list of changes and comments in
//! document order, and a footer that steps through it.
//!
//! Authors get marks that never rely on colour alone: a person is a 2 pt **solid** line in the
//! next author colour, the assistant a 2 pt **dotted** line in the `assistant` colour. The same
//! marks are used by the page (`wordcraft_layout::display::AuthorMarks`, built by [`marks`]).

use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{Document, Pos, Rgb};
use wordcraft_layout::display::{AuthorMark, AuthorMarks};

use crate::WordApp;
use crate::mac_controls as mc;
use crate::skin::{ASSISTANT_PAGE, AUTHOR_1_PAGE, AUTHOR_ROTATION, OptimoTokens};
use crate::theme::Tokens;

/// Every right-hand panel (Format, Review, later Assistant) is this wide.
pub const PANEL_W: f32 = 320.0;

/// The name an assistant's changes and comments are written under when Settings › Co-writer has
/// none.
pub const DEFAULT_ASSISTANT_NAME: &str = crate::assistant::DEFAULT_NAME;

thread_local! {
    // The configured name (Settings › Co-writer, S4), copied from the settings each frame and when
    // they change, so code without the app can read it.
    static ASSISTANT_NAME: std::cell::RefCell<String> = std::cell::RefCell::new(DEFAULT_ASSISTANT_NAME.to_string());
}

/// Copy the configured assistant author name where [`assistant_author_name`] reads it.
pub fn set_assistant_name(name: &str) {
    ASSISTANT_NAME.with(|n| {
        let mut n = n.borrow_mut();
        if *n != name {
            *n = name.to_string();
        }
    });
}

/// The configured assistant author name (Settings › Co-writer, S4): the one place the Review
/// panel's marks and the command field read it.
pub fn assistant_author_name() -> String {
    ASSISTANT_NAME.with(|n| n.borrow().clone())
}

thread_local! {
    // Settings › Co-writer S1 and a usable provider, copied each frame by `assistant_link::poll`.
    static ASSISTANT_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Copy whether an assistant can work where [`assistant_enabled`] reads it.
pub fn set_assistant_enabled(on: bool) {
    ASSISTANT_ON.set(on);
}

/// Whether an assistant is switched on (Settings › Co-writer, S1) with a provider it can use (a
/// key where one is needed). While false, the command field hides its Ask rows and the context
/// menu its "Ask…".
pub fn assistant_enabled() -> bool {
    ASSISTANT_ON.get()
}

thread_local! {
    // Names assistants wrote under without the "AI · " prefix (Settings › Co-writer's legacy
    // list, persisted on this Mac), copied with the configured name.
    static LEGACY_NAMES: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(vec![DEFAULT_ASSISTANT_NAME.to_string()]);
}

/// Copy the legacy names where [`assistant_names`] reads them.
pub fn set_legacy_names(names: Vec<String>) {
    LEGACY_NAMES.with(|n| *n.borrow_mut() = names);
}

/// The names the legacy rule recognises besides the "AI · " prefix: the configured name and the
/// names assistants wrote under before the prefix (handed to the engine too,
/// `assistant_link::sync_policy`).
pub fn assistant_names() -> Vec<String> {
    let mut v = vec![assistant_author_name()];
    LEGACY_NAMES.with(|n| v.extend(n.borrow().iter().cloned()));
    v
}

/// Whether `author` is an assistant: the one place the UI decides it, with the engine's rule
/// (`wordcraft_engine::authorship::is_assistant_name`: the "AI · " prefix, or a legacy name).
pub fn is_assistant(author: &str) -> bool {
    wordcraft_engine::authorship::is_assistant_name(author, &assistant_names())
}

/// The assistant's name where the dotted mark (or the word "Assistant") already says it is one:
/// without the "AI · " prefix. Review rows, the panel header, the toolbar toggle, the Ask rows.
pub fn shown_assistant_name() -> String {
    wordcraft_engine::authorship::display_name(&assistant_author_name()).to_string()
}

/// What the page and panel call an author with no name.
fn shown_name(author: &str) -> String {
    if author.trim().is_empty() { "Author".to_string() } else { author.trim().to_string() }
}

/// An author of a change or comment, with the colour slot they were given.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthorInfo {
    pub name: String,
    pub assistant: bool,
    /// Position among the people (assistants don't count): picks the colour.
    pub person: usize,
}

/// Authors in order of first appearance (revisions, then comments), so colours stay put while
/// changes are accepted. Several assistants (if the name is ever a list) share one colour.
pub fn authors(doc: &Document) -> Vec<AuthorInfo> {
    let mut out: Vec<AuthorInfo> = Vec::new();
    let names = doc.revisions.iter().map(|r| r.author.as_str()).chain(doc.comments.values().map(|c| c.author.as_str()));
    for n in names {
        let name = shown_name(n);
        if out.iter().any(|a| a.name == name) {
            continue;
        }
        let assistant = is_assistant(&name);
        let person = out.iter().filter(|a| !a.assistant).count();
        out.push(AuthorInfo { name, assistant, person });
    }
    out
}

fn info_for(list: &[AuthorInfo], name: &str) -> AuthorInfo {
    let name = shown_name(name);
    list.iter().find(|a| a.name == name).cloned().unwrap_or_else(|| {
        let assistant = is_assistant(&name);
        AuthorInfo { name, assistant, person: list.iter().filter(|a| !a.assistant).count() }
    })
}

/// The author's colour on the page (white in both appearances).
pub fn page_color(a: &AuthorInfo) -> Color32 {
    if a.assistant {
        ASSISTANT_PAGE
    } else if a.person == 0 {
        AUTHOR_1_PAGE
    } else {
        AUTHOR_ROTATION.get((a.person - 1) % AUTHOR_ROTATION.len()).copied().unwrap_or(AUTHOR_1_PAGE)
    }
}

/// The author's colour on a panel: the dark variants of `author_1` and `assistant`; the rotation
/// colours are lightened on dark panels to keep 4.5:1.
pub fn panel_color(a: &AuthorInfo, m: &OptimoTokens) -> Color32 {
    if a.assistant {
        m.assistant
    } else if a.person == 0 {
        m.author_1
    } else if m.dark {
        page_color(a).lerp_to_gamma(Color32::WHITE, 0.45)
    } else {
        page_color(a)
    }
}

fn to_rgb(c: Color32) -> Rgb {
    Rgb(c.r(), c.g(), c.b())
}

/// The marks the page draws for the document's authors: colour, line style and comment tint.
pub fn marks(doc: &Document) -> AuthorMarks {
    AuthorMarks(
        authors(doc)
            .iter()
            .map(|a| {
                let color = page_color(a);
                // `ASSISTANT_TINT` / `PERSON_TINT` for the first two, the author's own colour at
                // the person opacity for the rest.
                let alpha = if a.assistant { 23 } else { 31 };
                AuthorMark { author: a.name.clone(), color: to_rgb(color), dotted: a.assistant, tint: (to_rgb(color), alpha) }
            })
            .collect(),
    )
}

/// The initial on a comment tag in the page margin.
pub fn initial(author: &str) -> char {
    shown_name(wordcraft_engine::authorship::display_name(author)).chars().next().map(|c| c.to_uppercase().next().unwrap_or(c)).unwrap_or('A')
}

/// A 2 pt mark line from `a` to `b`: solid, or dotted (dots 2 pt across, 3.5 apart).
pub fn mark_line(p: &egui::Painter, a: Pos2, b: Pos2, color: Color32, dotted: bool) {
    if !dotted {
        p.line_segment([a, b], Stroke::new(2.0, color));
        return;
    }
    let d = b - a;
    let len = d.length();
    if !len.is_finite() || len < 0.5 {
        return;
    }
    let step = d / len;
    let n = ((len / 3.5).floor() as usize).clamp(1, 400);
    for i in 0..=n {
        let t = if n == 0 { 0.0 } else { len * i as f32 / n as f32 };
        p.circle_filled(a + step * t, 1.0, color);
    }
}

// ---------------------------------------------------------------------------------------------
// The list

/// Which items the list shows (R4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Changes,
    Comments,
}

/// How the markup shows on the page (R2). The engine knows markup on or off; Simple and
/// Original draw like All and No markup until the layout has those views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Show {
    #[default]
    All,
    Simple,
    Original,
    None,
}

const SHOW: [(Show, &str); 4] = [(Show::All, "All markup"), (Show::Simple, "Simple markup"), (Show::Original, "Original"), (Show::None, "No markup")];

#[derive(Clone, Debug)]
struct ChangeRow {
    author: String,
    date: String,
    deleted: String,
    inserted: String,
    /// "Formatted: Bold", "Moved to": for changes that are neither insertion nor deletion.
    note: String,
    a: Pos,
    b: Pos,
}

#[derive(Clone, Debug)]
struct Reply {
    author: String,
    date: String,
    text: String,
}

#[derive(Clone, Debug)]
struct CommentRow {
    id: u32,
    author: String,
    date: String,
    resolved: bool,
    anchor: String,
    text: String,
    replies: Vec<Reply>,
    start: Option<Pos>,
    end: Option<Pos>,
}

#[derive(Clone, Debug)]
enum Item {
    Change(ChangeRow),
    Comment(CommentRow),
}

impl Item {
    /// Where the item sits in the document (unanchored comments go last).
    fn at(&self) -> Option<&Pos> {
        match self {
            Item::Change(c) => Some(&c.a),
            Item::Comment(c) => c.start.as_ref(),
        }
    }
}

fn pos_of(v: &Value) -> Option<Pos> {
    serde_json::from_value(v.clone()).ok()
}

fn s_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// The text a comment is anchored on (its range, within the paragraph it starts in), with
/// object markers left out; and where that range ends.
fn anchor_text(doc: &Document, id: u32, start: &Pos) -> (String, Pos) {
    let Some(p) = doc.para(start.story, &start.path) else { return (String::new(), start.clone()) };
    let end = p
        .object_offsets()
        .into_iter()
        .find(|o| *o > start.off && matches!(p.object_at(*o), Some(InlineObject::CommentEnd { id: i }) if *i == id))
        .unwrap_or_else(|| p.len());
    let text: String = p.text.get(start.off..end).unwrap_or("").chars().filter(|c| *c != wordcraft_doc::para::OBJ).collect();
    (text, Pos { off: end, ..start.clone() })
}

const MAX_ITEMS: usize = 2_000;

/// Changes and comments in document order. A deletion directly followed by an insertion by the
/// same author (a replacement) is one row, accepted and rejected together.
fn build_items(app: &mut WordApp) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let changes = app.session.run("review.changes", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let mut rows: Vec<(String, ChangeRow)> = Vec::new();
    for c in changes.iter().take(MAX_ITEMS) {
        let (Some(a), Some(b)) = (c.get("start").and_then(pos_of), c.get("end").and_then(pos_of)) else { continue };
        let kind = s_of(c, "kind");
        let text = s_of(c, "text");
        let row = ChangeRow {
            author: s_of(c, "author"),
            date: s_of(c, "date"),
            deleted: if kind == "delete" { text.clone() } else { String::new() },
            inserted: if kind == "insert" { text } else { String::new() },
            note: if kind == "insert" || kind == "delete" { String::new() } else { s_of(c, "description") },
            a,
            b,
        };
        // A replacement: this insertion starts where the previous deletion by the same author ended.
        if let Some((k, last)) = rows.last_mut()
            && ((k == "delete" && kind == "insert" && !last.deleted.is_empty() && last.inserted.is_empty() && last.b == row.a)
                || (k == "insert" && kind == "delete" && !last.inserted.is_empty() && last.deleted.is_empty() && last.b == row.a))
            && last.author == row.author
        {
            last.deleted.push_str(&row.deleted);
            last.inserted.push_str(&row.inserted);
            last.b = row.b.clone();
            *k = "replace".to_string();
            continue;
        }
        rows.push((kind, row));
    }
    items.extend(rows.into_iter().map(|(_, r)| Item::Change(r)));

    // Comments, replies under the comment they answer.
    let list = app.session.run("review.comments", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let by_id = |id: u32| list.iter().find(|c| c.get("id").and_then(Value::as_u64) == Some(u64::from(id)));
    for c in list.iter().take(MAX_ITEMS) {
        if c.get("parent").is_some_and(|p| !p.is_null()) {
            continue;
        }
        let id = c.get("id").and_then(Value::as_u64).and_then(|i| u32::try_from(i).ok()).unwrap_or(0);
        let start = c.get("anchor").and_then(pos_of);
        let (anchor, end) = match &start {
            Some(s) => {
                let (t, e) = anchor_text(&app.session.doc, id, s);
                (t, Some(e))
            }
            None => (String::new(), None),
        };
        let mut replies = Vec::new();
        for r in &list {
            let Some(parent) = r.get("parent").and_then(Value::as_u64) else {
                continue;
            };
            // Replies to a reply join the thread.
            let root = {
                let mut p = parent;
                for _ in 0..16 {
                    match by_id(u32::try_from(p).unwrap_or(u32::MAX)).and_then(|x| x.get("parent")).and_then(Value::as_u64) {
                        Some(pp) => p = pp,
                        None => break,
                    }
                }
                p
            };
            if root == u64::from(id) {
                replies.push(Reply { author: s_of(r, "author"), date: s_of(r, "date"), text: s_of(r, "text") });
            }
        }
        items.push(Item::Comment(CommentRow {
            id,
            author: s_of(c, "author"),
            date: s_of(c, "date"),
            resolved: c.get("resolved").and_then(Value::as_bool).unwrap_or(false),
            anchor,
            text: s_of(c, "text"),
            replies,
            start,
            end,
        }));
    }
    // Document order; the sort is stable, so equal anchors keep changes before comments.
    items.sort_by(|x, y| match (x.at(), y.at()) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    items
}

/// Review shows an assistant edit made of several word-level pieces as one row (what the edit
/// replaced → what it put there), accepted and rejected together. The pieces are found the way
/// the co-writer finds them; after a restart (its runs are kept per session) they show
/// one row each.
fn group_assistant_edits(app: &WordApp, items: Vec<Item>) -> Vec<Item> {
    let groups = crate::cowriter::pending_groups(app);
    if groups.is_empty() {
        return items;
    }
    let mut items = items;
    for (author, parts, deleted, inserted) in groups {
        // This author's change rows, in document order, with their place in `items`.
        let idx: Vec<usize> =
            items.iter().enumerate().filter(|(_, it)| matches!(it, Item::Change(c) if c.author == author)).map(|(i, _)| i).collect();
        let list: Vec<(String, String, Pos, Pos)> = idx
            .iter()
            .filter_map(|i| match items.get(*i) {
                Some(Item::Change(c)) => Some((c.deleted.clone(), c.inserted.clone(), c.a.clone(), c.b.clone())),
                _ => None,
            })
            .collect();
        let Some((i, j)) = crate::cowriter::find_group(&list, &parts) else { continue };
        let (Some(&first), Some(&last)) = (idx.get(i), idx.get(j)) else { continue };
        let end = match items.get(last) {
            Some(Item::Change(c)) => c.b.clone(),
            _ => continue,
        };
        if let Some(Item::Change(c)) = items.get_mut(first) {
            c.deleted = deleted;
            c.inserted = inserted;
            c.b = end;
        }
        // Drop the other pieces (from the back, so indices stay valid).
        for k in idx.get(i + 1..=j).unwrap_or_default().iter().rev() {
            if *k < items.len() {
                items.remove(*k);
            }
        }
    }
    items
}

/// The items, recounted when the document changes.
fn items(app: &mut WordApp, ctx: &egui::Context) -> Arc<Vec<Item>> {
    let rev = app.session.rev();
    let id = egui::Id::new("margin_review_items");
    if let Some((r, v)) = ctx.data(|d| d.get_temp::<(u64, Arc<Vec<Item>>)>(id))
        && r == rev
    {
        return v;
    }
    let built = build_items(app);
    let v = Arc::new(group_assistant_edits(app, built));
    ctx.data_mut(|d| d.insert_temp(id, (rev, v.clone())));
    v
}

fn current(app: &WordApp, item: &Item) -> bool {
    let (a, b) = app.session.sel.ordered();
    match item {
        // The selection touches the change's range (a caret inside or at its edge counts).
        Item::Change(c) => a.story == c.a.story && a <= c.b && b >= c.a && (a != b || (a >= c.a && a <= c.b)),
        Item::Comment(c) => {
            app.canvas.balloon == Some(c.id)
                || match (&c.start, &c.end) {
                    (Some(s), Some(e)) => app.session.sel.focus.story == s.story && app.session.sel.focus >= *s && app.session.sel.focus <= *e,
                    _ => false,
                }
        }
    }
}

/// The tooltip of a comment's tag in the page margin: author and the start of the text.
pub fn tag_tip(c: &wordcraft_doc::Comment) -> String {
    format!("{}: {}", shown_name(&c.author), crate::panes::comment_stamp(&c.date))
}

/// Open the Review panel (closing the Format inspector) with comment `id` selected on the page.
pub fn open_on_comment(app: &mut WordApp, id: u32) {
    app.session.view.comments_pane = true;
    app.ui.optimo.inspector = false;
    let list = app.session.run("review.comments", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let Some(start) = list.iter().find(|c| c.get("id").and_then(Value::as_u64) == Some(u64::from(id))).and_then(|c| c.get("anchor")).and_then(pos_of)
    else {
        return;
    };
    let (_, end) = anchor_text(&app.session.doc, id, &start);
    let _ = app.run("select.range", json!({"anchor": start, "focus": end}));
}

/// Select an item on the page (R5: "Clicking the row selects the change in the page").
fn activate(app: &mut WordApp, item: &Item) {
    match item {
        Item::Change(c) => {
            let _ = app.run("select.range", json!({"anchor": c.a, "focus": c.b}));
        }
        Item::Comment(c) => {
            if let (Some(s), Some(e)) = (&c.start, &c.end) {
                let _ = app.run("select.range", json!({"anchor": s, "focus": e}));
            }
        }
    }
}

/// Accept or reject one change row: select its range, run the command.
fn resolve_change(app: &mut WordApp, c: &ChangeRow, accept: bool) {
    let _ = app.run("select.range", json!({"anchor": c.a, "focus": c.b}));
    let _ = app.run(if accept { "review.accept" } else { "review.reject" }, json!({}));
    app.canvas.want_focus = true;
}

// ---------------------------------------------------------------------------------------------
// The panel

/// The panel itself, docked on the right (320 wide).
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::right("margin_review")
        .exact_size(PANEL_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| content(app, ui));
}

fn content(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let m = OptimoTokens::get(ui.ctx());
    ui.spacing_mut().item_spacing = vec2(8.0, mc::ROW_GAP);
    let all = items(app, ui.ctx());
    let people = authors(&app.session.doc);
    let n_changes = all.iter().filter(|i| matches!(i, Item::Change(_))).count();
    let n_comments = all.iter().filter(|i| matches!(i, Item::Comment(_))).count();

    // R2: tracking and markup display.
    mc::section(ui, "Tracking", |ui| {
        let on = app.session.doc.settings.track_changes;
        if mc::switch(ui, "Track changes", on) {
            crate::widgets::run_from_ribbon(app, "review.trackChanges", json!({}));
        }
        let show_id = egui::Id::new("margin_review_show");
        let mut show = ui.data(|d| d.get_temp::<Show>(show_id)).unwrap_or_default();
        if !app.session.view.show_markup && matches!(show, Show::All | Show::Simple) {
            show = Show::None;
        }
        if app.session.view.show_markup && matches!(show, Show::Original | Show::None) {
            show = Show::All;
        }
        let labels: Vec<String> = SHOW.iter().map(|(_, l)| tl!(l).to_string()).collect();
        let cur = SHOW.iter().find(|(s, _)| *s == show).map_or("", |(_, l)| tl!(l));
        mc::labelled_row(ui, "Show", |ui| {
            let w = ui.available_width();
            if let Some(choice) = mc::popup_button(ui, "margin_review_show_popup", w, cur, &labels, false)
                && let Some((s, _)) = SHOW.iter().find(|(_, l)| tl!(l) == choice)
            {
                ui.data_mut(|d| d.insert_temp(show_id, *s));
                let value = match s {
                    Show::All => "allMarkup",
                    Show::Simple => "simpleMarkup",
                    Show::Original => "original",
                    Show::None => "noMarkup",
                };
                let _ = app.run("review.markup", json!({"value": value}));
            }
        });
    });

    // R3: people.
    if !all.is_empty() {
        mc::section(ui, "People", |ui| {
            let mut names: Vec<(AuthorInfo, usize, usize)> = Vec::new();
            for it in all.iter() {
                let (who, ch, co) = match it {
                    Item::Change(c) => (c.author.as_str(), 1, 0),
                    Item::Comment(c) => (c.author.as_str(), 0, 1),
                };
                let info = info_for(&people, who);
                match names.iter_mut().find(|(a, ..)| a.name == info.name) {
                    Some((_, x, y)) => {
                        *x += ch;
                        *y += co;
                    }
                    None => names.push((info, ch, co)),
                }
                // Replies count as the replier's comments.
                if let Item::Comment(c) = it {
                    for r in &c.replies {
                        let ri = info_for(&people, &r.author);
                        match names.iter_mut().find(|(a, ..)| a.name == ri.name) {
                            Some((_, _, y)) => *y += 1,
                            None => names.push((ri, 0, 1)),
                        }
                    }
                }
            }
            for (a, ch, co) in names {
                person_row(app, ui, &a, ch, co, &t, &m);
            }
        });
    }

    // R7 footer first (a bottom panel inside the panel), so the list gets what is left.
    let filter_id = egui::Id::new("margin_review_filter");
    let mut filter = ui.data(|d| d.get_temp::<Filter>(filter_id)).unwrap_or_default();
    let shown: Vec<&Item> = all
        .iter()
        .filter(|i| match filter {
            Filter::All => true,
            Filter::Changes => matches!(i, Item::Change(_)),
            Filter::Comments => matches!(i, Item::Comment(_)),
        })
        .collect();
    let cur_idx = shown.iter().position(|i| current(app, i));
    egui::Panel::bottom("margin_review_footer")
        .exact_size(48.0)
        .resizable(false)
        .frame(
            egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin { left: 12, right: 12, top: 0, bottom: 0 }).stroke(Stroke::new(1.0, t.border)),
        )
        .show(ui, |ui| footer(app, ui, &shown, cur_idx, &people, &t));

    // R4: "In this document" + All / Changes / Comments.
    egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 16, top: 20, bottom: 12 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = mc::ROW_GAP;
        ui.label(egui::RichText::new(tl!("In this document")).font(mc::font()).color(m.section_title));
        ui.add_space(10.0 - mc::ROW_GAP);
        let labels =
            [format!("{} {}", tl!("All"), all.len()), format!("{} {}", tl!("Changes"), n_changes), format!("{} {}", tl!("Comments"), n_comments)];
        let segs = [mc::Seg::Text(&labels[0]), mc::Seg::Text(&labels[1]), mc::Seg::Text(&labels[2])];
        let sel = match filter {
            Filter::All => 0,
            Filter::Changes => 1,
            Filter::Comments => 2,
        };
        if let Some(i) = mc::segmented_one(ui, "margin_review_filter_seg", &segs, Some(sel)) {
            filter = [Filter::All, Filter::Changes, Filter::Comments].get(i).copied().unwrap_or_default();
            ui.data_mut(|d| d.insert_temp(filter_id, filter));
        }
    });
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.min.y, Stroke::new(1.0, t.border));
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        if shown.is_empty() {
            egui::Frame::NONE.inner_margin(16).show(ui, |ui| {
                ui.set_width(ui.available_width());
                mc::hint(ui, if all.is_empty() { "No changes or comments yet." } else { "Nothing of this kind." });
            });
        }
        let mut clicked: Option<&Item> = None;
        for it in &shown {
            let cur = current(app, it);
            let hit = match it {
                Item::Change(c) => change_row(app, ui, c, cur, &people, &t, &m),
                Item::Comment(c) => comment_row(app, ui, c, cur, &people, &t, &m),
            };
            if hit {
                clicked = Some(it);
            }
        }
        if let Some(it) = clicked {
            activate(app, it);
        }
    });
}

/// One author: mark, name, counts; under an assistant the by-author accept / reject links (R3).
fn person_row(app: &mut WordApp, ui: &mut Ui, a: &AuthorInfo, changes: usize, comments: usize, t: &Tokens, m: &OptimoTokens) {
    let color = panel_color(a, m);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    mark_line(ui.painter(), pos2(r.min.x + 1.0, r.min.y + 3.0), pos2(r.min.x + 1.0, r.max.y - 3.0), color, a.assistant);
    ui.painter().text(pos2(r.min.x + 14.0, r.min.y + 9.0), egui::Align2::LEFT_CENTER, &a.name, mc::font(), t.text);
    let count = format!(
        "{} · {}",
        crate::i18n::fmt(if changes == 1 { tl!("{n} change") } else { tl!("{n} changes") }, &[("n", &changes.to_string())]),
        crate::i18n::fmt(if comments == 1 { tl!("{n} comment") } else { tl!("{n} comments") }, &[("n", &comments.to_string())])
    );
    ui.painter().text(pos2(r.min.x + 14.0, r.min.y + 27.0), egui::Align2::LEFT_CENTER, count, mc::font(), t.text_dim);
    if a.assistant && changes > 0 {
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.spacing_mut().item_spacing.x = 16.0;
            let label = crate::i18n::fmt(tl!("Accept {name}’s {n}"), &[("name", &a.name), ("n", &changes.to_string())]);
            if mc::link(ui, &label).clicked() {
                let _ = app.run("review.acceptAll", json!({"author": a.name}));
                app.canvas.want_focus = true;
            }
            if mc::link(ui, "Reject").clicked() {
                let _ = app.run("review.rejectAll", json!({"author": a.name}));
                app.canvas.want_focus = true;
            }
        });
    }
}

/// A 28 × 24 icon button.
pub(crate) fn icon_button(ui: &mut Ui, icon: &str, tip: &str, t: &Tokens) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 24.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.hover);
    }
    crate::icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, t.icon, t.icon);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!(tip)));
    resp.on_hover_text(tl!(tip)).clicked()
}

/// A row's frame: padding 12 / 16, the `checked` fill when current, a hairline below. Returns
/// whether the row (not one of its buttons) was clicked.
fn row_frame(ui: &mut Ui, current: bool, t: &Tokens, add: impl FnOnce(&mut Ui)) -> bool {
    let bg = ui.painter().add(egui::Shape::Noop);
    let w = ui.available_width();
    let out = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
        ui.set_width(w);
        egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: 12 }).show(ui, |ui| {
            ui.set_width(w - 32.0);
            ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
            add(ui);
        });
    });
    let r = out.response.rect;
    let fill = if current {
        t.checked
    } else if out.response.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().set(bg, egui::Shape::rect_filled(r, 0.0, fill));
    ui.painter().hline(r.x_range(), r.max.y, Stroke::new(1.0, t.border));
    out.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// The header line of a row: mark or icon, author, time, then `buttons` at the right.
fn header(ui: &mut Ui, who: &AuthorInfo, icon: Option<&str>, date: &str, t: &Tokens, m: &OptimoTokens, buttons: impl FnOnce(&mut Ui)) {
    let color = panel_color(who, m);
    ui.horizontal(|ui| {
        ui.set_height(24.0);
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 24.0), Sense::hover());
        match icon {
            Some(i) => crate::icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), i, color, color),
            None => mark_line(ui.painter(), pos2(r.center().x, r.min.y + 4.0), pos2(r.center().x, r.max.y - 4.0), color, who.assistant),
        }
        // An assistant without "AI · ": its dotted mark says so.
        let shown = if who.assistant { wordcraft_engine::authorship::display_name(&who.name) } else { who.name.as_str() };
        ui.label(egui::RichText::new(shown).font(mc::font()).color(t.text));
        ui.label(egui::RichText::new(crate::panes::comment_stamp(date)).font(mc::font()).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            buttons(ui);
        });
    });
}

fn change_row(app: &mut WordApp, ui: &mut Ui, c: &ChangeRow, cur: bool, people: &[AuthorInfo], t: &Tokens, m: &OptimoTokens) -> bool {
    let who = info_for(people, &c.author);
    let color = panel_color(&who, m);
    let mut accept = false;
    let mut reject = false;
    let clicked = row_frame(ui, cur, t, |ui| {
        header(ui, &who, None, &c.date, t, m, |ui| {
            // Right to left: reject first, so accept sits on its left.
            reject = icon_button(ui, "reject", "Reject", t);
            accept = icon_button(ui, "accept", "Accept", t);
        });
        let mut parts: Vec<(String, Deco)> = Vec::new();
        if !c.deleted.is_empty() {
            parts.push((clip(&c.deleted, 90), Deco::Strike));
        }
        if !c.deleted.is_empty() && !c.inserted.is_empty() {
            parts.push((" → ".to_string(), Deco::None));
        }
        if !c.inserted.is_empty() {
            parts.push((clip(&c.inserted, 90), Deco::Underline));
        }
        if !c.note.is_empty() {
            parts.push((clip(&c.note, 90), Deco::None));
        }
        excerpt(ui, &parts, color, who.assistant, t);
    });
    if accept {
        resolve_change(app, c, true);
        return false;
    }
    if reject {
        resolve_change(app, c, false);
        return false;
    }
    clicked
}

fn comment_row(app: &mut WordApp, ui: &mut Ui, c: &CommentRow, cur: bool, people: &[AuthorInfo], t: &Tokens, m: &OptimoTokens) -> bool {
    let who = info_for(people, &c.author);
    let color = panel_color(&who, m);
    let mut resolve = false;
    let mut action: Option<&'static str> = None;
    let reply_key = egui::Id::new(("comment_reply_open", c.id));
    let edit_key = egui::Id::new(("review_comment_edit", c.id));
    let clicked = row_frame(ui, cur, t, |ui| {
        header(ui, &who, Some("comment"), &c.date, t, m, |ui| {
            resolve = icon_button(ui, "resolve", if c.resolved { "Reopen" } else { "Resolve" }, t);
        });
        if !c.anchor.trim().is_empty() {
            let on = crate::i18n::fmt(tl!("On “{text}”"), &[("text", &clip(c.anchor.trim(), 60))]);
            ui.add(egui::Label::new(egui::RichText::new(on).font(mc::font()).color(t.text_dim)).truncate());
        }
        let ink = if c.resolved { t.text_dim } else { t.text };
        let editing = ui.data(|d| d.get_temp::<bool>(edit_key)).unwrap_or(false);
        if editing {
            crate::panes::comment_editor(app, ui, c.id, Some(mc::font()), false);
        } else {
            ui.add(egui::Label::new(egui::RichText::new(&c.text).font(mc::font()).color(ink)).wrap());
        }
        for r in &c.replies {
            let rw = info_for(people, &r.author);
            let rcolor = panel_color(&rw, m);
            let out = ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        egui::RichText::new(format!("{} · {}", rw.name, crate::panes::comment_stamp(&r.date))).font(mc::font()).color(t.text_dim),
                    );
                    ui.add(egui::Label::new(egui::RichText::new(&r.text).font(mc::font()).color(ink)).wrap());
                });
            });
            // The thread rule: 2 pt, in the replier's colour; dotted for the assistant.
            let rr = out.response.rect;
            mark_line(ui.painter(), pos2(rr.min.x + 1.0, rr.min.y + 1.0), pos2(rr.min.x + 1.0, rr.max.y - 1.0), rcolor, rw.assistant);
        }
        let replying = ui.data(|d| d.get_temp::<bool>(reply_key)).unwrap_or(false);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            if mc::link(ui, "Reply").clicked() {
                action = Some("reply");
            }
            if mc::link(ui, if editing { "Done" } else { "Edit" }).clicked() {
                action = Some("edit");
            }
            if mc::link(ui, "Delete").clicked() {
                action = Some("delete");
            }
        });
        if replying {
            let focus = ui.data_mut(|d| d.remove_temp::<bool>(egui::Id::new(("comment_reply_focus", c.id)))).unwrap_or(false);
            if crate::panes::reply_editor(app, ui, c.id, Some(mc::font()), focus) {
                ui.data_mut(|d| d.remove::<bool>(reply_key));
            }
        }
        let _ = color;
    });
    if resolve {
        let _ = app.run("review.resolveComment", json!({"id": c.id}));
        return false;
    }
    match action {
        Some("reply") => {
            ui.data_mut(|d| {
                d.insert_temp(reply_key, true);
                d.insert_temp(egui::Id::new(("comment_reply_focus", c.id)), true);
            });
            false
        }
        Some("edit") => {
            let editing = ui.data(|d| d.get_temp::<bool>(edit_key)).unwrap_or(false);
            if editing {
                crate::panes::commit_comment(app, ui.ctx(), c.id);
            }
            ui.data_mut(|d| d.insert_temp(edit_key, !editing));
            false
        }
        Some("delete") => {
            let _ = app.run("review.deleteComment", json!({"id": c.id}));
            false
        }
        _ => clicked,
    }
}

pub(crate) fn clip(s: &str, max: usize) -> String {
    let flat: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if flat.chars().count() <= max {
        return flat;
    }
    let mut out: String = flat.chars().take(max).collect();
    out.push('…');
    out
}

/// How an excerpt part is marked: deletions struck, insertions underlined.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Deco {
    None,
    Strike,
    Underline,
}

/// "deleted → inserted" in the author colour (R5): deletions struck, insertions underlined, both
/// dotted for the assistant. The lines are drawn rather than set as text format so they can be dotted.
pub(crate) fn excerpt(ui: &mut Ui, parts: &[(String, Deco)], color: Color32, dotted: bool, t: &Tokens) {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = ui.available_width();
    for (text, deco) in parts {
        let c = if *deco == Deco::None { t.text_dim } else { color };
        job.append(text, 0.0, egui::TextFormat { font_id: mc::font(), color: c, ..Default::default() });
    }
    let galley = ui.painter().layout_job(job);
    let (r, _) = ui.allocate_exact_size(galley.size().max(vec2(0.0, 18.0)), Sense::hover());
    ui.painter().galley(r.min, galley.clone(), t.text);
    // Part boundaries in characters, to find each glyph's part.
    let mut ends = Vec::new();
    let mut n = 0usize;
    for (text, _) in parts {
        n += text.chars().count();
        ends.push(n);
    }
    let part_of = |i: usize| ends.iter().position(|e| i < *e).unwrap_or(parts.len().saturating_sub(1));
    let mut i = 0usize;
    for row in &galley.rows {
        // Per part, the x extent on this row.
        let mut spans: Vec<(usize, f32, f32)> = Vec::new();
        for g in &row.glyphs {
            let p = part_of(i);
            i += 1;
            let (x0, x1) = (row.pos.x + g.pos.x, row.pos.x + g.pos.x + g.advance_width);
            match spans.last_mut() {
                Some((sp, _, e)) if *sp == p => *e = x1,
                _ => spans.push((p, x0, x1)),
            }
        }
        for (p, x0, x1) in spans {
            let y = match parts.get(p).map(|(_, d)| *d) {
                Some(Deco::Strike) => row.pos.y + row.size.y * 0.55,
                Some(Deco::Underline) => row.pos.y + row.size.y - 2.0,
                _ => continue,
            };
            let y = r.min.y + y;
            // Trailing spaces of a wrapped row aren't underlined.
            mark_line_thin(ui.painter(), pos2(r.min.x + x0, y), pos2(r.min.x + x1, y), color, dotted);
        }
    }
}

/// A 1 pt text decoration line, solid or dotted.
fn mark_line_thin(p: &egui::Painter, a: Pos2, b: Pos2, color: Color32, dotted: bool) {
    if !dotted {
        p.line_segment([a, b], Stroke::new(1.0, color));
        return;
    }
    let len = (b - a).length();
    if !len.is_finite() || len < 0.5 {
        return;
    }
    let step = (b - a) / len;
    let n = ((len / 3.0).floor() as usize).min(400);
    for k in 0..=n {
        p.circle_filled(a + step * (len * k as f32 / n.max(1) as f32), 0.7, color);
    }
}

/// R7: previous / next, "3 of 7", and the Accept all menu.
fn footer(app: &mut WordApp, ui: &mut Ui, shown: &[&Item], cur: Option<usize>, people: &[AuthorInfo], t: &Tokens) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let n = shown.len();
        let go = |app: &mut WordApp, to: usize| {
            if let Some(it) = shown.get(to) {
                activate(app, it);
            }
        };
        if icon_button(ui, "chevronUp", "Previous", t) && n > 0 {
            go(app, cur.map_or(n - 1, |i| (i + n - 1) % n));
        }
        if icon_button(ui, "chevronDown", "Next", t) && n > 0 {
            go(app, cur.map_or(0, |i| (i + 1) % n));
        }
        ui.add_space(8.0);
        let label = match cur {
            Some(i) => crate::i18n::fmt(tl!("{n} of {total}"), &[("n", &(i + 1).to_string()), ("total", &n.to_string())]),
            None => crate::i18n::fmt(tl!("{total} items"), &[("total", &n.to_string())]),
        };
        ui.label(egui::RichText::new(label).font(mc::font()).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let changes = shown.iter().filter(|i| matches!(i, Item::Change(_))).count();
            let mut by: Vec<String> = Vec::new();
            for it in shown {
                if let Item::Change(c) = it {
                    let a = shown_name(&c.author);
                    if !by.contains(&a) {
                        by.push(a);
                    }
                }
            }
            let _ = people;
            let mut act: Option<(bool, Option<String>)> = None;
            mc::popup_button_with(ui, egui::Id::new("margin_review_accept_all"), 120.0, tl!("Accept all"), |ui| {
                ui.set_min_width(220.0);
                if changes > 0 && mc::menu_row(ui, tl!("Accept all shown"), false).clicked() {
                    act = Some((true, None));
                    ui.close();
                }
                for a in &by {
                    let label = crate::i18n::fmt(tl!("Accept all by {name}"), &[("name", a)]);
                    if mc::menu_row(ui, &label, false).clicked() {
                        act = Some((true, Some(a.clone())));
                        ui.close();
                    }
                }
                if changes > 0 && mc::menu_row(ui, tl!("Reject all"), false).clicked() {
                    act = Some((false, None));
                    ui.close();
                }
            });
            if let Some((accept, author)) = act {
                let id = if accept { "review.acceptAll" } else { "review.rejectAll" };
                let params = author.map_or(json!({}), |a| json!({"author": a}));
                let _ = app.run(id, params);
                app.canvas.want_focus = true;
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_rotate_colours_and_the_assistant_is_apart() {
        let mut d = Document::new();
        for (a, k) in [("Ana", 0), ("Claude", 1), ("Bo", 2)] {
            let _ = k;
            d.revisions.push(wordcraft_doc::Revision::new(wordcraft_doc::RevisionKind::Insert, a, "2026-10-13T10:00:00Z"));
        }
        let list = authors(&d);
        assert_eq!(
            list.iter().map(|a| (a.name.as_str(), a.assistant, a.person)).collect::<Vec<_>>(),
            [("Ana", false, 0), ("Claude", true, 1), ("Bo", false, 1)]
        );
        assert_eq!(page_color(&list[0]), AUTHOR_1_PAGE);
        assert_eq!(page_color(&list[1]), ASSISTANT_PAGE);
        assert_eq!(page_color(&list[2]), AUTHOR_ROTATION[0], "the second person gets the first rotation colour");
        let m = marks(&d);
        assert_eq!(m.0.iter().map(|x| x.dotted).collect::<Vec<_>>(), [false, true, false], "only the assistant is dotted");
    }

    #[test]
    fn assistant_name_match_is_case_and_space_tolerant() {
        assert!(is_assistant(" claude "));
        assert!(!is_assistant("Claudia"));
        assert_eq!(initial("claude"), 'C');
        assert_eq!(initial(""), 'A');
    }

    /// A replacement is one row; comments carry their replies; the by-author command leaves
    /// other people's changes alone.
    #[test]
    fn items_merge_replacements_and_group_replies() {
        use crate::Services;
        use wordcraft_engine::Session;
        let mut a = WordApp::new(Session::new(Document::from_text("Rent is obvious reason here")), Services::default());
        a.run("review.trackChanges", json!({"value": true})).unwrap();
        a.run("file.setAuthor", json!({"name": "Ana"})).unwrap();
        a.run("select.text", json!({"text": "obvious"})).unwrap();
        a.run("text.insert", json!({"text": "main"})).unwrap();
        a.run("select.text", json!({"text": "here"})).unwrap();
        a.run("review.newComment", json!({"text": "where?"})).unwrap();
        a.run("file.setAuthor", json!({"name": "Claude"})).unwrap();
        a.run("review.reply", json!({"id": 0, "text": "there"})).unwrap();
        a.run("select.text", json!({"text": "Rent"})).unwrap();
        a.run("text.insert", json!({"text": "Cost"})).unwrap();
        let list = build_items(&mut a);
        let kinds: Vec<String> = list
            .iter()
            .map(|i| match i {
                Item::Change(c) => format!("change {} {}>{}", c.author, c.deleted, c.inserted),
                Item::Comment(c) => format!("comment {} {}", c.author, c.replies.len()),
            })
            .collect();
        assert_eq!(kinds, ["change Claude Rent>Cost", "change Ana obvious>main", "comment Ana 1"], "{kinds:?}");
        a.run("review.acceptAll", json!({"author": "Claude"})).unwrap();
        assert_eq!(build_items(&mut a).len(), 2, "only Ana's replacement and comment are left");
    }
}
