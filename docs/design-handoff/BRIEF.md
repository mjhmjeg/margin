# Margin — design brief for fine-tuning

Date: 2026-10-13 · Build: fork of WordCraft 0.6.0 · Repo: github.com/mjhmjeg/margin

## 1. What Margin is

Margin is a **word processor for the Mac**. It opens, edits and saves Word documents (.docx), plus
ODT, RTF, Markdown, HTML and PDF export. Under the hood it is **WordCraft**, an open-source,
pure-Rust reimplementation of Microsoft Word. Margin keeps WordCraft's document engine and replaces
its Office-style interface with one that feels native on macOS.

**Who it is for:** people on a Mac who receive and send Word documents and want a calm, Pages-like
editor instead of Word's ribbon. Free and open source (MIT / Apache-2.0), published on GitHub.

**Design direction so far (approved):** flat, quiet, Mac-native. Flat surfaces with hairline borders,
Apple system blue as the single accent, **one interface type size (13 pt, regular weight)** with
hierarchy by colour only, generous spacing, light and dark following macOS. Liquid Glass was tried
and rejected in favour of flat.

## 2. Screens (in `screens/`)

All screenshots are 1440 × 900 pt windows at 2× (2880 × 1800 px). Light and dark where it matters.

| File | Screen | Notes |
|---|---|---|
| `01-home-templates-*` | Home › New: "Choose a template" | Shown at launch and on File › New (⌘N). Five templates; cards show the real first page. |
| `02-home-recent-*` | Home › Recent | Recent documents as first-page thumbnails, folder · date. |
| `09-home-save-as-light` | Home › Save As | Upstream page reused inside the Margin frame. |
| `10-home-export-light` | Home › Export | Upstream page, list of export formats. |
| `11-home-more-options-light` | Home › More… › Options | Upstream settings page (many sections). |
| `03-editor-text-*` | Editor, Format inspector › Text | Main working screen. |
| `04-editor-layout-*` | Inspector › Layout | Indents, columns, pagination. |
| `05-editor-document-*` | Inspector › Document (Pages) | Format, paper, orientation, margins, header/footer. |
| `06-editor-pageless-*` | Pageless (Google Docs-style) | One continuous sheet; text width Narrow/Medium/Full. |
| `07-editor-outline-*` | Outline sidebar open | Upstream Navigation pane (headings, pages, search results). |
| `08-editor-no-inspector-light` | Inspector hidden | Focus on the page. |
| `12-toolbar-table-menu-light` | Toolbar › Table menu | Popover style. |
| `13-toolbar-zoom-menu-light` | Toolbar › Zoom menu | Popover style. |
| `14-dialog-paragraph-light` | A dialog (Format › Paragraph…) | **Upstream dialog, not yet restyled** — representative of ~50 dialog types. |
| `15-classic-ribbon-light` | View › Classic Ribbon (⌃⌘R) | The original WordCraft ribbon, kept as an option. Out of scope for redesign. |

Earlier design references: `docs/mac/mockup.html` (editor, approved) and `docs/mac/home-mockup.html`
(home). Before/after of the original WordCraft UI: `docs/mac/before/`.

**Not in the screenshots: the macOS menu bar** (it is drawn by macOS, not the app):
Margin (About, Settings…, Services, Hide, Quit) · File (New… ⌘N, Open… ⌘O, Close ⌘W, Save ⌘S,
Save As… ⇧⌘S, Export as PDF…, Print… ⌘P) · Edit (Undo, Redo, Cut, Copy, Paste, Paste and Match
Style, Select All, Find…, Find and Replace…, Spelling and Grammar) · Insert (Table…, Image…, Shape ›,
Link… ⌘K, Page Break, Footnote, Table of Contents, Comment) · Format (Bold, Italic, Underline,
Strikethrough, Align Left/Center/Right/Justify, Bullets, Numbering, Paragraph Style ›, Font…,
Paragraph…) · Review (Track Changes, Accept, Reject, Next Change, New Comment, Word Count) · View
(Show Outline, Show Format Inspector, Show Ruler, Show Invisibles, Pageless, Dark Page, Focus, Zoom
In/Out, Actual Size, Fit Width, Classic Ribbon, Full Screen) · Window · Help (Search Commands…).

## 3. Current design system (the values in the build)

**Type:** Inter, 13 pt, regular (400) for all interface text. No bold, no size steps. Document
content uses the document's own fonts (not part of the UI).

**Colour (light / dark):**

| Token | Light | Dark | Used for |
|---|---|---|---|
| title bar, toolbar, status bar | #F6F6F6 | #2A2A2A | chrome |
| panel (inspector, sidebars) | #FAFAFA | #262626 | side panels |
| canvas (desk around pages) | #ECECEC | #1E1E1E | behind the page |
| border (hairline) | #E3E3E3 | #383838 | dividers |
| border strong | #D1D1D1 | #474747 | |
| input border | #8A8A8A | #7A7A7A | fields, segmented controls (min. 3:1 contrast) |
| text | #1D1D1F | #F2F2F2 | |
| text dim | #6E6E73 | #A1A1A6 | labels |
| group label (section titles, hints) | #A1A1A6 | #6E6E73 | |
| hover / pressed / checked | #EAEAEA / #DCDCDC / #E3E3E3 | #333 / #3D3D3D / #3A3A3A | |
| accent | #0A64D6 (Apple blue) | #3D9BFF | selection, primary button, active segment |
| on accent (text on blue) | #FFFFFF | #0B1A2E | |
| page | #FFFFFF | #FFFFFF (Dark Page is a separate option) | |

**Sizes (pt):** toolbar 36 high (controls centred on the 28 pt traffic-light band) · inspector 272
wide, 16 side padding, 14 section padding, rows 26 high · segmented controls and fields radius 6 ·
toolbar buttons 30 × 24, radius 6 · home sidebar 220 wide, rows 30 high, pill radius 6 · template
cards 168 wide, paper radius 3, soft shadow, selection ring 2.5 pt with 3 pt gap · status bar 26 high.

**Icons:** line icons drawn in code (`crates/ui-egui/src/icons.rs`), 17 pt in the toolbar.

## 4. What we CAN change

- **Every colour, size, spacing, radius, border and shadow** in the tables above (they are tokens).
- **Layout of Margin's own screens:** toolbar (which buttons, order, grouping), the Format inspector
  (tabs, sections, controls, order), the Home screen (sidebar, cards, header, footer), the outline
  sidebar's look, the status bar contents.
- **Which controls appear where**: anything the engine can do can get a button (~600 commands exist:
  formatting, styles, lists, tables, images, shapes, track changes, comments, references, mail
  merge, page setup…).
- **Copy / labels** in Margin's screens.
- **Icons**: new or changed icons, as long as they are simple vector shapes we can draw in code.
- **The app icon** (`assets/app-icon/margin.svg`, a placeholder): any original artwork, delivered as SVG.
- **Popovers and menus inside the window** (toolbar menus, combo lists): look and spacing.
- **Dialogs**: a common restyle via tokens (padding, fields, buttons) is possible; per-dialog
  redesign is possible but costs work per dialog (there are ~50).

## 5. What we can NOT change (or only at high cost)

- **Truly native Mac controls.** The UI is drawn by egui (immediate-mode, custom-drawn), not AppKit.
  We match the look of Mac controls; we can't use real NSButton/NSTextField, real NSToolbar,
  native popovers or sheets.
- **Translucency / blur of window content** (Liquid Glass, vibrancy behind the document): not
  supported. Flat surfaces only.
- **Traffic lights position.** macOS fixes them in a 28 pt band at the top-left; the toolbar must
  share that band (no taller unified toolbar with centred lights).
- **The macOS menu bar look.** It is the system's; we choose items, names, order and shortcuts only.
- **System font (SF Pro).** Can't be bundled (licence). Interface font is Inter (open); any other
  font must be open-licensed.
- **No Microsoft / Apple / Adobe assets.** No icons, images or templates copied from Word, Pages or
  any proprietary app (clean-room rule of the WordCraft project). Everything original or openly licensed.
- **Accessibility floors** (enforced by tests): text on blue ≥ 4.5:1 contrast, field outlines
  ≥ 3:1 against their background. Very faint hairlines on inputs or white text on bright blue fail.
- **Document rendering.** How the page and its content look (fonts, Word layout, the page itself) is
  the document, not the UI. We can change the desk around it and the page shadow/edge.
- **The Classic ribbon.** Kept as WordCraft's original for people who want it; not redesigned.
- **Upstream settings pages** (Options, Info, Print, Export) are WordCraft's; restyle via tokens yes,
  full redesign = bigger work.

## 6. What we'd like back from Claude Design

1. Annotated frames for the screens above (at least: Home templates, Home recent, editor with
   inspector Text/Layout/Document, Pageless, outline open), light and dark.
2. Changed values as a **token list** (name → light / dark value) and sizes in **pt**, so they map
   1:1 onto `crates/ui-egui/src/skin.rs` and `mac_ui.rs` / `mac_home.rs`.
3. Any new icons as **SVG** (simple strokes, 24 × 24 grid).
4. A short list of layout changes per screen ("move X above Y", "drop Z").

Constraints from the product owner: whitespace over density; no cramped controls; one type size
(hierarchy by colour, underline rather than bold for selected); no visible "AI" or generic
dashboard look; dark mode nice to have, never a blocker.
