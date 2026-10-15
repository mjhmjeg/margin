# Optimo — design handoff for Claude Code (supersedes parts of HANDOFF.md)

From: Claude Design, 14 Oct 2026 · Repo: `wordcraft-mac` (the app is now **Optimo**, Sursilvan for “optimised”)
Design canvas (private to Martijn), page “Optimo — minimal + co-writer”: https://claude.ai/code/artifact/20ed961d-3ac9-49ee-bbee-5bcada84f0da
Each frame has orange pins (W1, Y2, T3…). This file uses the same IDs and is complete without the canvas.

**How this relates to `HANDOFF.md`:** phases 1–2 there (tokens, shared controls) are done (`../phase-1`, `../phase-2`) and stay. The dialogs and popovers phase, the Review panel (R), Settings › Assistant (S) and the icons still apply. This file **replaces**: the editor toolbar and status bar (E1–E5, E14), the command field (K1–K4), the Assistant chat panel (A1–A5) and the authorship view (V). Where the two files disagree, this one wins.

---

## 0. What Optimo is

A word processor where **you write every sentence**. The AI is a **co-writer and mentor** in the margin: it asks questions, points at things, checks facts and offers options, then waits. It is not a ghostwriter.

### Five product rules, enforced in code (not only in the UI)

1. **No generation of body text.** No verb may insert more than one sentence of new prose. The only exception is “I’m stuck › rough first line” (one line, opt-in).
2. **Options, never replacements.** The user's version is always the first option and the default. Esc keeps it.
3. **Everything the AI adds is marked and reversible:** a tracked change or comment under the AI author (flagged as AI), drawn with the dotted mark (§2).
4. **Facts need sources.** Research may only insert a footnote/citation or a quote with its source attached. No uncited sentences.
5. **Format for me never changes words.** Assert that the document's plain text before and after is identical (whitespace-normalised). If the check fails, abort and show the error.

---

## 1. Phase O0 — rename

- App name, bundle display name, menu bar ("Optimo › About Optimo…"), window titles, About box, Settings, README, the default AI author name ("Claude (co-writer)" stays as an example; the label in UI is “Co-writer”).
- The app icon from `margin-app-icon*.svg` stays as a placeholder. Name files `optimo-app-icon*.svg` (the artwork is unchanged; a new mark is out of scope).
- Settings text: “Let a co-writer work in Optimo”.

---

## 2. Phase O1 — the Optimo window (W)

The window is the page on the desk. **Default state shows nothing but the page.**

- **W1 Top bar** 36 pt, same colour as the desk (`canvas_bg`), **no bottom border**. Traffic lights on the left; document title **centred** + “Edited” in `text_dim`.
- **W2 Right side: only two controls**
  - **“Anything ⌘Y”** pill: 28 high, radius 14, `field_bg`, 1 pt `border_strong`, search icon, label “Anything” in `text_dim`, a `⌘Y` key cap (`checked` fill, radius 9). Click = ⌘Y.
  - **Co-writer** toggle: `assistant` icon in the assistant colour + “Co-writer”. Checked fill while any co-writer view is open in the margin.
  - **Remove from the window:** insert/comment/track icons, zoom button, Format/Review/Assistant panel toggles, ruler, outline toggle, the status bar row. Everything stays reachable from ⌘Y, the menu bar and shortcuts.
- **Format inspector, Review panel, Outline** open only via ⌘Y / menu / shortcut. Defaults: Format ⌥⌘I, Review ⌥⌘V, Outline ⌥⌘O; confirm free keys in the registry. They keep the v1 design and slide in from the right/left; closing returns to the bare page.
- **Desk layout at 1440 wide:** page 816 centred (x 312–1128). Right desk margin x 1148–1420 (**co-writer notes**), left desk margin x 24–288 (**structure map, before thumbnail**). On narrower windows the margins collapse into a 320 pt overlay sheet on the right.
- **W3 Quiet markers:** while you type, the co-writer shows nothing. If notes exist, the right margin shows small markers beside their paragraph: a 14 pt dotted line + “2 notes” in `text_dim`. Click opens them.
- **W4 Bottom line** 32 pt, no border, `text_dim`: left “370 words · Your words 97 %” + a 48 × 4 bar. Right: “Focus”.
  - “Your words” = share of characters authored by people. Phase 1 counts pending AI changes only (that data exists today). Persistent authorship after accept is the large engine task from v1 (V); do it later.
- Ruler: off by default (View › Show Ruler).

**Done when:** a fresh window shows only lights, title, two controls, the page and the bottom line, in light and dark mode.

---

## 3. Phase O2 — ⌘Y Anything (Y) — replaces the command field (K)

- **Y1** ⌘Y anywhere opens a centred box over the page: 640 wide, top 92, radius 12, `popover_*`, shadow `0 12 40 rgba(0,0,0,.16)`.
  - Input row 48 high: search icon + placeholder “Do, find, format, go to — or ask your co-writer”.
  - On the right, a scope chip: “Selection: 1 sentence” / “Whole document” (`checked` fill, radius 10).
  - **Note:** in Word, ⌘Y is Repeat. Map Repeat to ⇧⌘Y or drop it, and say so in release notes.
- **Y2 Empty query** shows groups in this order:
  1. **Co-writer:** Alternatives ⌥⌘A, Read as a reader… ⌥⌘R, Structure ⌥⌘S, Research… ⌥⌘F, Check facts and names, I’m stuck, **Format for me ⌥⌘T**. Each row is 32 high: name (168 wide), a one-line description in `text_dim`, shortcut on the right.
  2. **Format the selection:** a row of chips (26 high, radius 6, `field_bg`, `border_strong`): **B**, *I*, Heading 2, Quote, Link…, Comment, More format… (opens the Format inspector).
  3. **Go to:** headings, with “Heading · empty” for empty sections.
  Footer: “Type to search 600 commands · ↩ run · esc close”.
- **Typing** filters one ranked list across co-writer verbs, all registry commands (name + menu path + real shortcut), headings and recent files. Recent and frequent items rank first.
- **Free text that matches nothing** (or reads like a sentence) becomes the top row: “Ask the co-writer: “…”” with the current scope. Results come back as **notes in the margin**, never as inserted text (rule 1).
- Help › Search Commands focuses ⌘Y. Remove the old Search Commands dialog.

---

## 4. Phase O3 — co-writer infrastructure

- **Model connection and Settings** as in v1 §9 (S1–S6): provider (Anthropic / Mistral / on this Mac / OpenAI-compatible), Keychain key, author name, permissions, MCP outside access. Renamed to “Co-writer”.
- **One verb runner:** `cowriter::run(verb, scope, options) -> Vec<Output>`. Allowed outputs per verb are fixed:

| Verb | Allowed outputs | Lands as |
|---|---|---|
| Read as a reader (R) | comments, with optional one-click options | Comments by the AI author |
| Check facts and names | comments with options (“Use 34” / “Keep”) | Comments; the chosen option becomes a tracked change |
| Alternatives (A) | ≤ 3 rewrites of **the selected sentence(s)** | Nothing until picked; then one tracked replacement |
| Structure (S) | heading moves, heading inserts, comments | Tracked moves / tracked heading insert (heading text only) |
| Research (F) | findings with source; footnotes; quotes | Footnote + source entry, or a marked quote; never plain prose |
| I’m stuck (U) | questions (comments); optionally 1 first line | Comments; the first line as a tracked insert, opt-in |
| Format for me (T) | style/structure/typography changes only | Tracked formatting changes; text-identity assert |

- **Guards in the runner** (unit-tested): the output-type whitelist per verb, the max new words per verb (0 for R/Check/S/T, 1 sentence for A per selected sentence, 1 line for U), and the plain-text identity check for T.
- **Mark language** (unchanged from v1): people = solid line, AI = **2 pt dotted** line in `assistant` (#5B4FCF / dark #ADA3FF), with `assistant_tint` rgba(91,79,207,.09) for anchors. Use it on margin notes, legends, underlines of changed words and change bars.
- **Notes in the desk margin** (shared component for R, Check, F, U):
  - 268 wide; 12 pt left padding behind a 2 pt dotted `assistant` rule; 6 pt gaps.
  - Line 1: the kind (“Check”, “Question”, “Strong line”, “Rhythm”) in the assistant text colour (#5B4FCF light = 5.0:1 on the desk, #ADA3FF dark).
  - Line 2: the note in `text`. Line 3: action links in `accent`.
  - Notes align vertically with their anchor and push down on collisions. The anchor in the page gets the tint + dotted underline.
  - Notes are stored as .docx comments under the AI author, so Word users see them.
- **Presence while working:** the bottom line shows “Co-writer is reading… · Stop”. The v1 page caret (A5) is optional.

---

## 5. Phase O4 — Format for me (T) — build first among the verbs

This verb can be **mostly rule-based**. The model is only needed to guess what kind of document it is (Look).

- **T1** In ⌘Y under Co-writer, ⌥⌘T. Scope = the selection if there is one, otherwise the whole document.
- **T2** The left desk margin shows a 264 wide **“Before” thumbnail** of the page (rendered before the change) with “hold ⌥ to compare”. Holding ⌥ temporarily shows the old formatting in the page.
- **T3** The right margin opens with: dotted mark + “Formatted by your co-writer”, then a `checked` row with a lock icon: **“Your words: 0 changed · 412 kept”** (from the identity check).
- **T4 Look:** segmented Clean · Classic · Report · Letter. The default is guessed (“Reads like a handbook”). Each look is a **real set of paragraph styles** (Title, Subtitle, Heading 1–3, Body, List Bullet/Number, Table, Quote, Header/Footer), written to the .docx style table. Switching looks re-applies instantly (one undo step).
- **T5 What changed** list, each row with **Show** (selects and flashes those spots):
  - bold / ALL-CAPS / underlined short lines followed by body text → Heading 1/2/3 (by position and length; first line → Title, second short line → Subtitle)
  - lines starting with `- * • –` or `1) 1. a)` → real bulleted / numbered lists
  - runs of lines aligned with tabs or 2+ spaces with consistent columns → a table (first row = header)
  - mixed fonts → Look fonts (keep monospace for code-like runs)
  - empty paragraphs used as spacing → removed; spacing comes from styles
  - double spaces, straight quotes → typographic quotes for the document language, `...` → `…`, ` - ` → ` – `
  - header with document title, page numbers in the footer
  Note: typographic quotes, ellipses and dashes change characters but not words. Normalise them before the identity check.
- **T6** Options: Keep my colours (on), Add a table of contents (off), Number the headings (off).
- **T7** Buttons: Undo all · One by one (opens Review listing each tracked formatting change) · **Keep** (primary).
- Anything that would change words (typos, a wrong heading text) is **not** done here. It becomes a “Check” note instead.

---

## 6. Phase O5 — Alternatives (A)

- **A1** Opens **inside the page**, under the selected sentence. It is a popover card that inherits the page width, radius 10.
  - Header in `text_dim`: dotted mark + “Other ways to say it · your words kept where they work”.
  - A **Tone** popup on the right, default “Like the rest of the doc”. Others: plainer, warmer, more formal, shorter.
- **Rows** 34 high: label column 64 (`text_dim`), then the sentence in the **document font**.
  - Row 1 is **“Yours”** (selected, `checked`, hint “esc”).
  - Then **Sharper / Plainer / Warmer** with keys 1–3.
  - Only the **changed words** get the assistant colour + dotted underline.
- **A2/A3 Footer:** “1–3 use one · ⇥ more like one · ✎ edit before using” and “Arrives as a suggestion”. Picking one creates **one tracked replacement** by the AI author.
- **Prompt rule:** keep the user's wording where possible; same meaning; never add claims.

---

## 7. Phase O6 — Read as a reader (R) and Check

- **R1** Top of the right margin: “Read as” + reader popup (A first-time member, The board, A funder, A sceptic, Custom…). Below it, a summary in `text_dim`: “4 notes · 2 questions · 1 strong line”.
- **R2 Check:** facts against allowed files (e.g. members.xlsx) with one-click options (“Use 34” / “Keep thirty”).
- **R3 Question:** where that reader would stumble. Actions: Reply · Alternatives (opens A on that sentence) · Done.
- **Strong line:** what works and should be kept. Action: Thanks.
- **R4 Rhythm / consistency:** e.g. parallel structure in lists.
- **Cap:** at most 1 note per paragraph and 8 per run, ranked by value. The mentor is selective, not a linter.

---

## 8. Phase O7 — Structure (S)

- **S1 Left desk margin “Shape”** (264 wide): “6 sections · 370 words”, then one block per heading. Each block shows the heading, a one-line purpose written by the co-writer (`text_dim`) and a 3 pt length bar (`border_strong`, width proportional to words). The current section gets the `checked` fill. Click scrolls; dragging reorders (tracked move).
- **S2 Suggestions between blocks**, with a dotted rule: “Move “Membership” here? Readers decide to join before they read the rules.” Actions: Move it · Not now.
- **S3** Empty sections say “Empty — get unstuck” (link to U).
- **S4 Missing:** a dashed block (1.5 pt dashed `border_strong`, radius 8): “Costs — a reader will look for what membership costs.” Actions: Add heading (**heading only**) · Not needed.

---

## 9. Phase O8 — Research and fact check (F)

- **F1** Right margin. A query field (from ⌘Y “Research…” or a selected claim) and a scope line: “Web · your folder Studio · 3 sources”. Folders come from Settings permissions.
- **F2 Findings:** one sentence in the source's own terms; the source as title · site · year in `text_dim`. Actions: **Cite as footnote** (footnote + a sources list at the end) · **Quote** (inserted with quote style + citation) · Open. When sources disagree, show both.
- **F3 “Claims in this document”:** numbers, dates and names. A filled dot means a mismatch with a source; a hollow dot means verified or unknown, and unknowns say “no source found; your call”.
- Network: through the model provider's tools or a configurable search API. Never paste uncited text (rule 4).

---

## 10. Phase O9 — I’m stuck (U)

- Triggered from ⌘Y, or offered in Structure for empty sections. The caret goes into the empty paragraph and a **grey italic placeholder** shows: “Start anywhere — the questions on the right are there if you want them.” It disappears on the first keystroke.
- **U1 “Questions only you can answer”:** 3–5 questions under a dotted rule. Clicking one pins it as a comment at the caret.
- **U2 “From your notes”:** relevant earlier comments, minutes or linked files, quoted with their origin.
- **U3** In `text_dim`: “Still stuck? Show a rough first line to argue with”. It inserts **one** tracked line by the AI author.

---

## 11. Build order and effort

| Phase | What | Needs model | Effort |
|---|---|---|---|
| O0 | Rename to Optimo | no | S |
| O1 | Optimo window (W) | no | M |
| O2 | ⌘Y Anything (Y), remove old Search Commands | no (ask row hidden until O3) | M |
| O3 | Co-writer runner, guards, margin-note component, Settings | yes | M–L |
| O4 | Format for me (T) | Look guess only | M |
| O5 | Alternatives (A) | yes | S–M |
| O6 | Read as a reader + Check (R) | yes | M |
| O7 | Structure (S) | yes | M |
| O8 | Research (F) | yes + search | L |
| O9 | I’m stuck (U) | yes | S |
| later | Review panel (v1 R), persistent authorship (v1 V) | — | — |

One PR per phase. Screenshots at 1440 × 900, light and dark, go into `../phase-O<n>/`, as in phases 1–2.

## 12. Assets

- `icons/`: new `format`, `structure`, `alternatives`, `reader`, `stuck` (24 grid, 1.5 stroke), plus all earlier icons. `assistant` (caret + tag) is the Co-writer icon.
- `tokens.json` is unchanged. Optimo also uses `canvas_bg` for the top and bottom bars, and `border_strong` #C9C9CC light / #4A4A4C dark for the ⌘Y pill and length bars. Add these two values if you want them exact.

## 13. Open questions for Martijn

1. Is ⌘Y Repeat moving to ⇧⌘Y OK?
2. Is the default Look list right (Clean, Classic, Report, Letter), and should users be able to save their own look?
3. Research sources: web search via the model provider, or a separate search API key in Settings?
