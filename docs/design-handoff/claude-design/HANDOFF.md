# Margin — design handoff for Claude Code

From: Claude Design pass, 13 Oct 2026 · For: the `wordcraft-mac` repo (Margin fork of WordCraft 0.6.0)

Inputs: `../BRIEF.md`, `../REVIEW-AND-AI.md`, `../screens/`, `../review/`.
Design canvas (private to Martijn): https://claude.ai/code/artifact/20ed961d-3ac9-49ee-bbee-5bcada84f0da
Every frame there has orange pins (E1, H3, R5…). This file lists the same IDs, so you can work from this file alone.

In this folder:
- `tokens.json` — every colour (light / dark), shadow and size (pt).
- `icons/*.svg` — 38 icons, 24 × 24 grid, stroke 1.5, round caps/joins, `currentColor`.
- `margin-app-icon.svg` (≥ 64 pt) and `margin-app-icon-small.svg` (≤ 32 pt) replace `assets/app-icon/margin.svg`.

---

## 0. Ground rules (do not break)

- egui, custom-drawn. No AppKit controls, no blur/vibrancy. Traffic lights stay in the 28 pt band.
- **Inter 13 pt, weight 400, for all interface text.** No bold, no size steps. Hierarchy is colour only. The selected tab gets a 2 pt underline. Glyph buttons (B / I / U / S) are icons and can use weight or slant.
- Accessibility tests must keep passing: text on accent ≥ 4.5:1, field outlines ≥ 3:1. Every new pair in `tokens.json` was checked; ratios are in §1.
- Classic ribbon (⌃⌘R) stays as it is. Upstream pages (Save As, Export, Options, Print) get token changes only, except the new Settings › Assistant section (§9).
- No Microsoft/Apple/Adobe assets. All icons here are original.
- Work in the phases below, one PR per phase, with before/after screenshots at 1440 × 900 light + dark.

---

## 1. Phase 1 — tokens (`crates/ui-egui/src/skin.rs`)

Rename keys to match `skin.rs`; values are in `tokens.json`. What actually changes:

| Token | Light | Dark | Change |
|---|---|---|---|
| section_title | #1D1D1F | #F2F2F2 | **Changed** — replaces `group_label` (#A1A1A6) for section titles. The old value is 2.5:1 on #FAFAFA and fails the 4.5:1 floor. |
| hint | #6E6E73 | #98989D | **New** — helper sentences under controls (also replaces `group_label`). |
| segment_track | #F3F3F5 | #1E1E1E | **New** — segmented control and switch-off track. |
| segment_chip | #FFFFFF | #4A4A4C | **New** — selected segment. **Replaces the accent fill on segments.** |
| chip_shadow | see json | see json | New |
| field_bg | #FFFFFF | #1E1E1E | New — fields, popups, secondary buttons |
| focus_ring | accent @ 45 % | accent @ 50 % | New — 3 pt outside the control |
| page_shadow | 3 layers | 2 layers | New — replaces the flat page border |
| popover_bg / _border / _shadow | #FFFFFF / #D1D1D1 | #2E2E2E / #474747 | New — popovers, menus, dialogs |
| author_1 | #B8327A | #EC86BC (panel text) | New (review) |
| assistant | #5B4FCF | #ADA3FF (panel text) | New — reserved for AI authors, never assigned to a person |
| assistant_tint / person_tint | rgba(91,79,207,.09) / rgba(184,50,122,.12) | same | New |

Everything else (chrome, panel, canvas, borders, input_border, text, text_dim, hover, pressed, checked, accent, on_accent) stays.

Contrast (WCAG): section_title on panel 16.1 · text_dim on panel 4.9 · text_dim on segment_track 4.6 · hint (dark) on panel 5.3 · input_border on field 3.5 light / 3.9 dark · on_accent on accent 5.5 light / 6.1 dark · accent link on panel 5.3 / 5.3 · author_1 on white 5.5 · assistant on white 5.9.
Add those pairs to the existing accessibility tests.

The page stays white in dark mode, so author colours on the page are the light values in both modes. Panels use the dark variants.

**Done when:** tokens compile, contrast tests pass, nothing else in the layout has moved yet.

---

## 2. Phase 2 — shared controls (one function each, used everywhere)

| Control | Spec (pt) |
|---|---|
| Toolbar button | 32 × 28, radius 6, icon 17. Toggled = `checked` fill. Group divider: 8 · 1 × 16 `border_strong` · 8 |
| Underline tabs | height 44 (40 in sidebars and dialogs), 28 apart, centred. Selected = `text` + 2 pt `accent` underline; others `text_dim` |
| Segmented | height 28, `input_border` 1 pt outline, radius 6, `segment_track` fill, 2 pt inner padding. Selected = `segment_chip` (radius 4) + `chip_shadow`, label `text`; others `text_dim`. Icon segments at 16 pt |
| Popup button | one control, 28 high, radius 6, `field_bg`, `input_border`. Label left with 10 pt padding, `popup` icon (14 pt, `text_dim`) inside at the right. **Remove the separate arrow box** |
| Number field | 28 high, value right-aligned in `text`, unit after it in `text_dim` ("28 pt") |
| Labelled row | grid: label column 80 (`text_dim`), 8 gap, control fills the rest. Rows 28, 8 apart, 10 after a section title |
| Section | padding 20 top/bottom, 16 sides, hairline `border` below. Title in `section_title` |
| Checkbox / radio | 14, radius 4 / round, `accent` fill when on, `on_accent` tick |
| Switch | 30 × 18, knob 14. On = `accent`; off = `segment_track` + 1 pt `input_border` ring |
| Buttons | 28 high, min 80 wide, radius 6, 16 side padding. Secondary = `field_bg` + `input_border`. Primary = `accent` + `on_accent` |
| Popover / menu | `popover_*` tokens, radius 8, padding 6, rows 26, row radius 5, 6 below its button. Hover row = `accent` pill + `on_accent` text. Current value = check icon in a 20 pt left column |

---

## 3. Phase 3 — editor (`mac_ui.rs`)

**Toolbar** (36 high, controls centred on the traffic-light band)
- E1 Title = document name + "Edited" in `text_dim` while unsaved.
- E2 **Remove the paragraph-style combo** (it's in the inspector). Insert group: Table, Image, Shape, Link.
- E3 Second group: Comment, Track changes (toggle), More (…).
- E4 Zoom popover button "100%" + `chevron_down`. It's the only zoom control (see E14).
- K1 Command field in the toolbar centre (Phase 6).
- R1 / E5 Right end: three panel toggles, `inspector` **Format** · `review` **Review** (+ count badge) · `assistant` **Assistant**. One panel open at a time; the open one is `checked`.

**Format inspector** (272 wide)
- E6 Tabs Text / Layout / Document = underline tabs.
- E7 Section titles in `section_title`; row labels in `text_dim`.
- E8 Paragraph style and font = popup buttons.
- E9 Font section: family popup; then one row with the size field (80 wide) and the B I U S segmented control; then a new **Colour** row (swatch + "Automatic" popup).
- E10 Alignment = 4 icon segments (`align_*`); lists = None / Bullets / Numbers. Neutral chip, no blue.
- E11 Spacing: Line (×), Before (pt), After (pt) as number fields.
- E12 Bottom of the Text tab: links "Font…" and "Paragraph options…" in `accent` (open the existing dialogs).
- E15 Layout › Columns: segmented 1 / 2 / 3 + "Gap" field (disabled at 1 column).
- E16 Layout › Pagination: checkboxes Keep with next, Keep lines together, Page break before, Widow control.
- E17 Document › Format: Pages / Pageless + hint text.
- E18 Document › Margins: 2 × 2 grid (Top, Bottom / Left, Right), label above field.
- E19 Document › Headers and footers: switches Header, Footer, Page numbers; "Edit" link on rows that are on; checkbox Different first page.

**Desk and page**
- E13 `page_shadow` instead of the flat border; 12 pt gap below the ruler. Vertical ruler off by default (View › Show Ruler still toggles both). The horizontal ruler spans only the page width, with margins shaded `hover`.
- E20 Pageless: the sheet fills the desk, no shadow, no ruler. Text width Narrow 560 / Medium 720 / Full (sheet − 2 × 64). The Document tab shows "Text width" instead of Page and Margins.

**Status bar** (28 high)
- E14 Left: `Page 1 of 2 · 357 words · English (United States)` with `text_dim` dots, then the review status (R9). Right: Pages/Pageless icon segmented control (22 high) + Focus. **Remove the zoom slider and the second 100 %.**

**Outline sidebar**
- E21 220 wide, `panel_bg`, search field on top (12 pt margin).
- E22 Headings / Pages / Results as underline tabs (40 high).
- E23 Rows 28, indent 14 pt per level, `chevron_down` / `chevron_right` on parents, the current section gets the `checked` pill.

---

## 4. Phase 4 — Home (`mac_home.rs`)

- H1 Sidebar runs full height; traffic lights sit in its top 36 pt. No title strip above the content.
- H2 Rows 30, icon 16 + 10 gap: New (`new_document`), Recent (`recent`), Open… (`open`). Selected = `checked` pill.
- H3 A "Document" group label (`text_dim`) above Save As… / Print… / Export…. Shown in `text_dim` and inert when no document is open.
- H4 "More…" becomes **Settings** (`settings` icon), pinned to the bottom of the sidebar, and opens Options.
- H5 Header 52 high: title left; search field (240, `search` icon) right; hairline below.
- H6 Template grid: cards 168, column gap 36, row gap 32, 40 inset. The selection ring is unchanged. Double-click creates.
- H7 Footer 60: "Open…" secondary on the left; on the right, Cancel (only if a document window is open) and Create (primary).
- H8 Recent header adds "Sort" popup (Date) and a grid/list segmented control (`view_grid` / `view_list`).
- H9 Recent captions on two lines: name (`text`, no extension unless names clash), then "folder · date" (`text_dim`).
- H10 Recent footer: "Open…" left, "Open" primary right (enabled when a thumbnail is selected). Empty state: "Documents you open will appear here." + Open….

---

## 5. Phase 5 — dialogs and popovers

Dialogs (one shared frame, fixes all ~50):
- D1 No title bar or close box. The title is the first line, centred, in `text`. Esc = Cancel. Radius 10, padding 20, `popover_*`.
- D2 Dialog tabs use the underline tab component.
- D3 Group labels in `section_title`; form labels right-aligned in a 112 pt column, 8 pt to the control.
- D4 Fields, radios and checkboxes use the Phase 2 controls.
- D5 Buttons bottom-right, 12 apart: Cancel (secondary), then OK (primary). Extra actions (Tabs…) go bottom-left.
- D6 Units follow the document's unit setting, not hard-coded inches.

Popovers:
- P1 Popover frame as in Phase 2.
- P2 Insert Table = 10 × 8 hover grid of 16 pt cells, 3 apart. Hovered cells are filled with `accent` @ 18 % and outlined in `accent`. The label "3 × 4 table" sits below, then a separator and "More…".
- P3 Menu rows as in Phase 2.
- P4 Zoom menu: 50 / 75 / 100 / 125 / 150 / 200 %, separator, Fit Width, Fit Page.

---

## 6. Phase 6 — command field (promotes Help › Search Commands)

- K1 Field 300 × 28 in the toolbar centre: `search` icon, placeholder "Search commands or ask Claude", shortcut hint `⇧⌘/` (the macOS "search menus" shortcut; ⌘K stays Link). Focused = `accent` border + `focus_ring`.
- K2 The results popover (480 wide, rows 28) hangs from the field. It searches the existing command registry (same data as Search Commands). Each row shows the name in `text`, the menu path in `text_dim` and the **real** shortcut right-aligned. The shortcuts in the mockup are placeholders. Rank recent and frequent commands first. The top hit is highlighted; ↩ runs it, ↑↓ move, Esc closes. Footer hint row in `text_dim`.
- K3 The last row is always "Ask Claude “…”" (`assistant` icon in the `assistant` colour). If nothing matches, or the query looks like a sentence, the ask row moves to the top. ⇥ toggles between Commands and Ask. Hide the ask row when the assistant is off in Settings.
- K4 With a selection, the ask is scoped to it (keep the selection drawn while the popover is open). Quick asks: Shorten, Fix grammar and spelling, Make it plainer, Check facts against a file….
- Also: the selection's context menu gets "Ask Claude…", which opens this field pre-scoped. Drop the floating format bar idea.
- Rename Help › Search Commands… to focus this field.

**Done when:** every command in the registry is reachable from the field, and the old Search Commands dialog can be removed.

---

## 7. Phase 7 — Review panel (UI over existing commands)

Panel 320 wide, `panel_bg`, opened by the Review toggle (R1).
- R2 Top section: "Track changes" switch; "Show" popup (All markup / Simple markup / Original / No markup).
- R3 "People": one row per author (from `review.changes` + comments). Each row has the author's mark, name and "n changes · n comments". Under an AI author: "Accept Claude’s n" / "Reject" (accept/reject filtered by author).
- **Mark language:** person = 2 pt **solid** line in the author colour; assistant = 2 pt **dotted** line in `assistant`. Use the same marks for the page margin bar, legends, thread rules and insertion underlines. Never rely on colour alone.
- Author colours: first person `author_1`; more people rotate #1F7A3A, #B45309, #0E7490, #9D174D. `assistant` is reserved for AI authors (detect via the AI author flag / configured author name).
- R4 "In this document" + segmented All n / Changes n / Comments n. A single list in document order **replaces the WordCraft Comments pane**.
- R5 Change row (padding 12 / 16, hairline between rows): mark, author, time (`text_dim`), then `accept` / `reject` icon buttons (28 × 24). Below, an inline excerpt "deleted → inserted" in the author colour (strike / underline; dotted underline for the assistant). Clicking the row selects the change in the page; the current row gets the `checked` fill.
- R6 Comment row: `comment` icon in the author colour, author, time, `resolve` button. Below: the anchor ("On “thirty strangers”") in `text_dim`, the text, and replies indented under a 2 pt rule (dotted for the assistant). Links: Reply, plus any action the assistant proposed ("Yes, change it").
- R7 Footer 48: previous / next (`chevron_up` / `chevron_down`), "3 of 7" in `text_dim`, and an "Accept all" menu button (Accept all shown · Accept all by <author> · Reject all…).
- R8 Page: insertions underlined and deletions struck in the author colour; change bar 22 pt left of the text (solid / dotted). Comment anchors get a `person_tint` / `assistant_tint` background. Comment markers sit in the right page margin as 24 × 22 tags with the author's initial: filled for a person, outlined for the assistant.
- R9 Status bar: "Tracking changes" with a 7 pt `accent` dot (replaces "Track Changes: On"). Click toggles.

---

## 8. Phase 8 — Assistant panel (needs a model connection + Keychain)

Panel 320 wide, opened by the Assistant toggle.
- A1 Header: dotted mark + author name ("Claude (co-writer)"). Line 2 in `text_dim`: what it is doing ("Working in “What we agree on” · 1 of 4"). A "Stop" secondary button shows while it works.
- A2 History is a work log, not chat bubbles. "You · time" in `text_dim` + instruction. The assistant's reply sits under a 2 pt dotted `assistant` rule, with "Claude · time" in `text_dim` and a short report.
- A3 A reply ends with a bordered group (radius 8) of its suggestions, rows 36, each with accept / reject, and a "Show in Review" link. **Nothing is applied until accepted.** Every AI edit is a tracked change under the AI author.
- A4 Composer: "Scope" popup (Selection / This section / Whole document); 76 pt instruction box; `send` button (`accent`). A permanent line with the `lock` icon: "Edits arrive as tracked suggestions".
- A5 Presence on the page while working: a 2 pt caret in `assistant` with a name tag ("Claude", 11 pt on `assistant`, white text — the one exception to 13 pt, same as annotation tags), a faint `assistant_tint` over the block it is working on, and a dotted margin bar. Remove all of it when it stops.

---

## 9. Settings › Assistant (new section in Options)

- S1 Master switch "Let an assistant work in Margin". Off hides the Assistant panel and the ask row in the command field.
- S2 Provider radios: Anthropic (Claude) · Mistral · On this Mac (local model) · Other (OpenAI-compatible endpoint). Model popup per provider.
- S3 API key in the macOS Keychain. The field shows only the last 4 characters, with Change… / Remove.
- S4 Author name (written into changes and comments). The colour and dotted mark are fixed for assistants.
- S5 Permissions: "Always suggest as tracked changes" is checked and disabled (required). "Add and reply to comments" (on). "Read files in these folders (read-only)" + folder list with + / −. Map these to the MCP sandbox settings.
- S6 "Allow assistants outside Margin to connect" (MCP from Claude Desktop / Claude Code), with "n connected now · Disconnect".

---

## 10. Phase 9 — authorship view (large; engine work, do last)

- V1 View › Show Authorship (also in the command field) shows a pill at the top of the desk: legend (People solid · Claude dotted) + Hide.
- V2 AI-written text keeps `assistant_tint` + a dotted `assistant` underline **after acceptance**. Person text stays plain.
- V3 Paragraphs with ≥ 10 % AI text get a chip in the left desk margin: "Claude 41 %".
- V4 Status bar shows the document split: "People 88 % · Claude 12 %".
- Needs: authorship kept on runs after accept, stored as Margin-only metadata in .docx (Word ignores it). **Cheaper first step:** ship V1–V4 for pending changes only, then add persistence.

---

## 11. Icons → `crates/ui-egui/src/icons.rs`

All on a 24 grid, stroke 1.5, round caps and joins. Draw at 17 pt in the toolbar, 16 in sidebars and segments, 14–15 in fields and rows.
sidebar_left, inspector, table, image, shape, link, comment, track_changes, more, chevron_down / up / right, popup, search, focus, view_pages, view_pageless, new_document, recent, open, save_as, print, export, settings, align_left / center / right / justify, view_grid, view_list, review, assistant (collaborator caret + name tag — **no sparkle or robot**), accept, reject, resolve, send, lock, stop.

## 12. Open questions for Martijn

1. Is `⇧⌘/` OK for the command field, or do you prefer ⌘J?
2. Should the Review and Assistant panels be 320 wide (as drawn), or match Format at 272?
3. Should the Assistant toolbar label show the configured author name ("Claude") instead of "Assistant"?
