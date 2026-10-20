# Decisions (fork-specific, append-only)

## 2026-10-12 — Fork WordCraft and restructure its UI for macOS (option B)

**Context.** storytold/wordcraft is a 5-day-old, Apache/MIT, pure-Rust Word clone. Engine and UI are
separate crates (`ui-egui` is L6, swappable; colours come from `theme::Tokens`). The UI copies the
Office 365 ribbon and reads as a Windows app on a Mac. Goal: public product, outside users.

**Decision.** Option B: keep egui, replace the ribbon with a Mac-style toolbar + right format
inspector + native menu bar. Ribbon stays reachable (View › Classic) to keep upstream merges small.
New UI goes into new files; edits to upstream files stay minimal. Contribute back as PRs.
Upstream merged weekly.

**Alternatives rejected.**
- A, re-theme only: cheap but leaves the Windows structure.
- C, native SwiftUI shell over the engine: best Mac feel, months of work on an engine that is 5 days old.

**Risks.** Upstream ships daily (v0.3→v0.5 in 3 days); merge conflicts in `ui-egui/src/lib.rs`
(206 KB) and `ribbon.rs`. egui draws its own widgets, so text input, accessibility and menus are
never truly native (menu bar via native NSMenu is the exception).

## 2026-10-12 — Product name: Margin

**Decision.** Martijn picked "Margin" (from Feder, Folio, Margin, Quire, Leaf).
**Known near-clashes.** An early-stage macOS markdown notes app called Margin (peerlist.io), and
"The Margin", an iPad writing tracker. Neither is a word processor. No formal trademark search done.

## 2026-10-12 — Mockup feedback (phase 1)

- Accent: Apple system blue (#0A64D6). Red kept only as a mockup toggle.
- Layout: outline left, format inspector right (Text / Layout / Document) — approved.
- Pageless option (Google Docs style) in Document tab + View menu, with text width Narrow / Medium / Full.
  Built on the existing `view.webLayout` engine view, so no engine change; text width is a UI setting.
  Print and PDF export still paginate.

## 2026-10-12 — Liquid Glass direction (mockup, pending approval)

**Real in the app:** macOS window vibrancy (NSVisualEffectView behind a transparent eframe window),
floating rounded sidebar and inspector, capsule toolbar groups, translucent fills, light rims.
**Not real:** blur of the document under floating chrome, refraction/specular "liquid" effects —
would need custom wgpu shaders; unlikely upstream. Mockup's flat style kept as a toggle.

## 2026-10-12 — Flat, not Liquid Glass

Martijn chose flat after trying the glass variant. Glass stays only as a mockup toggle.
Inspector spacing increased: one checkbox per row, labels above segmented controls, 16 px section padding.

## 2026-10-12 — One type size for the whole UI

All interface text is 13 px, weight 400 (menus, toolbar, inspector, sidebar, status bar). Hierarchy by
colour only (text / dim / faint). Document content keeps its own fonts and sizes.

## 2026-10-12 — Phase 2 implementation notes

- All skin code in `crates/ui-egui/src/skin.rs`; upstream files only get hooks (theme.rs token
  constructors + font helpers, chrome.rs title bar, ribbon.rs scope guard, one test guard).
- Process-wide switch `skin::on()` (default on). Classic ribbon keeps upstream type sizes
  (`skin::Classic`), because 13 pt clips its labels; colours stay Margin's there.
- Accessibility tests upstream (#655 on-accent 4.5:1, #657 input borders 3:1) kept and passing:
  input borders #8A8A8A / #7A7A7A instead of Mac's fainter hairline; dark mode uses a lighter blue
  (#3D9BFF) with near-black text on blue buttons instead of Apple's #0A84FF + white (3.65:1).

## 2026-10-12 — Follow the macOS appearance by default

Upstream defaults to a fixed Light interface; Martijn's Mac is in Dark mode and only the page went
dark (Switch Modes = dark page, Word semantics). Margin defaults the interface theme to System.
Switch Modes keeps Word's meaning (page only), like Pages' separate dark-document option.

## 2026-10-12 — Phase 3: Mac layout

- New file `crates/ui-egui/src/mac_ui.rs`; `lib.rs` swaps title bar + ribbon for it when the skin is
  on and Classic ribbon is off. Prefs live in `UiState.margin` (saved in ui.json).
- Outline sidebar = upstream's Navigation pane (headings + search), toggled from the toolbar.
- Pageless = `view.webLayout` at 100 % zoom, column width set via `session.view.web_width`
  (Narrow 520 pt, Medium 680 pt, Full = window). No engine change.
- Toolbar lives in the 28 pt traffic-light band: AppKit fixes the lights there and moving them
  needs `unsafe`, which the workspace forbids.
- Upstream tests run with Classic ribbon on (they test the ribbon layout); Mac layout has its own
  tests in `mac_ui.rs`.

## 2026-10-12 — Phase 4: native menu bar

- muda =0.21.2 (Tauri's menu crate; safe API over AppKit, reuses objc2 0.6 / objc2-app-kit 0.3
  already in the build). Pinned like fmv-macos-events. Mac-only, no default (GTK) features.
- Menu contents are data in `ui-egui/src/mac_menu.rs` (testable, control channel `ui.menu`).
- A native key equivalent eats the keystroke before winit, so items whose shortcut the app already
  handles replay it as a synthetic egui event (Keys / Copy / Cut / Paste): identical behaviour in
  the page and in text fields. Other items run their command.
- Rejected: predefined AppKit Cut/Copy/Paste (send `copy:` to the first responder, which winit's
  view doesn't implement); no shortcuts on Edit items (not Mac-like).

## 2026-10-12 — Phase 5: packaging

- `packaging/macos/package.sh` takes branding from env (APP_NAME, BUNDLE_ID, APP_ICNS, FILE_PREFIX,
  COPYRIGHT; WordCraft defaults) and notarizes with a `notarytool` keychain profile
  (NOTARY_PROFILE) so no Apple password passes through env or the agent. Upstreamable.
- Margin icon: original SVG (page + one blue margin rule), rendered by packaging/macos/margin-icon.sh.
- App name "Margin" in window title, menus, eframe app name; bundle id ai.spert.margin.
- README.md is Margin's; upstream's kept as README-WordCraft.md.
- Version follows upstream (0.6.0) for now.

## 2026-10-13 — Pages-style Home screen

Martijn: sidebar New, Recent, Open…, Save As, Print, Export, More… (Info, Save, Options); Recent as
page thumbnails; shown at launch and on File › New (⌘N). Built in `ui-egui/src/mac_home.rs`; the
document pages are upstream's, reused. Only in the Mac layout (Classic ribbon keeps the Backstage,
and upstream's Backstage tests run in Classic). Recent thumbnails: one render per frame, files over
20 MB show an icon.

## 2026-10-13 — Claude Design open questions (HANDOFF.md §12)

1. Command field shortcut: Martijn prefers ⌘J "if free". It is not: ⌘J = Justify (Word's, in
   Format menu). Decided 2026-10-13: **⌘/** (free, two keys; Notion/Slack use it for commands).
2. Review and Assistant panels: 320 pt (as drawn).
3. Toolbar label: "AI Assistant".

## 2026-10-13 — Assistant A2: providers and key

- New crate `wordcraft-assistant` (L0, standalone). Two connectors: Anthropic Messages, OpenAI-compatible chat completions; seven presets (Mistral pre-selected, Anthropic, OpenAI, Gemini, OpenRouter, On this Mac, Other). Base URLs checked against the providers' docs (cited in `crates/assistant/src/lib.rs`).
- HTTP: `ureq` 3 with rustls (ring, webpki roots), blocking, on a background thread; 15 s timeout. Keys: `keyring` 3.6 (macOS Keychain, Windows Credential Manager, Linux kernel keyring, which lasts until logout). Both are target-gated off wasm32; the browser build has no assistant network.
- Rejected: `keyring` 4 (new store architecture, more surface for no gain here), Secret Service on Linux (C libdbus or a second zbus), tokio/reqwest.
- Commands are `ui.assistant` (settings; `apiKey` write-only), `ui.assistant.test`, `ui.assistant.models`. Outside assistants are refused all three. A key is never sent over plain http to another computer.

## 2026-10-13 — Assistant A3: Assistant panel and the edit protocol

- **Edit protocol: strict JSON, no provider tool-calling** (tool-calling differs across Mistral, OpenAI, Gemini, OpenRouter and local servers). The model answers `{"report": …, "edits": [{"kind": "replace" | "insert" | "comment", "quote", "with", "note"}]}`, report first so it streams into the work log. Parsed defensively (bare, fenced, or embedded in prose; at most 50 edits). Each `quote` must occur exactly once inside the scope (within one paragraph, not across a tracked change or object); missing, ambiguous, overlapping and out-of-scope edits are skipped with a work-log line. Applied last-to-first, each as its own command through `Session::run_as_assistant` (same deny list as outside assistants), limited further to `select.range`, `text.delete`, `text.insert`, `review.newComment` (`engine/src/assistant_edits.rs`). A replacement is a tracked deletion followed by a tracked insertion; the person's selection, tracking state and Review pane come back afterwards.
- **Untrusted document:** sent between `BEGIN DOCUMENT <nonce>` / `END DOCUMENT <nonce>` lines (random per ask); the system prompt says text there is data and its instructions must not be followed.
- **Scopes:** Selection; This section (from the nearest heading at or above the caret to the next heading of the same or higher level); Whole document. Capped at 40,000 characters, cut at a paragraph end (inside the first paragraph if it alone is longer); the work log says how many words were sent.
- **Suggestions in the panel:** accept/reject find their change live by (author, deleted, inserted) and run the person's own `review.accept` / `review.reject`; a comment suggestion's accept resolves it, reject deletes it. Changes resolved elsewhere show "Resolved".
- **Work log:** per document, in memory for the session (not saved). Times are UTC HH:MM, like Review's stamps.
- **Stop:** sets a flag the stream checks between lines and drops the receiver; nothing from that answer is applied.
- Toolbar toggle is labelled with the configured author name (owner decision), shown only while `assistant_enabled()`.
- "Check facts against a file…" stays hidden until folder access (A4).

## 2026-10-13 — Assistant A4: folder access (read-only) and fact-check against a file

- **One allow-list, two users.** Settings › Assistant › "Read files in these folders" is the MCP sandbox's read root with the person's list as the roots (`file_access::AutomationRoots::for_app`: several read roots, writes left alone). `control::handle_outside` runs every outside request inside it, so any command that reads a path (`file.open`, `insert.picture`, `insert.textFromFile`, …) is held to the list; empty list = no file reads for outside assistants (a change from before, when a connected app was unconfined for reads). The built-in assistant uses the same check for its source file. Headless `wordcraft-cli mcp` keeps its own `--automation-read-root` / `--automation-write-root` flags, unchanged; Settings does not touch it.
- **Comments switch.** `Session::assistant_comments` (set from Settings) makes `denied` refuse `review.newComment`, `review.reply` and `review.editComment` to assistants; the built-in assistant's comment edits are dropped with a log line. Removing its own comments stays allowed.
- **Fact check (K4).** The file's text goes as a second nonce-fenced untrusted block (`BEGIN SOURCE <nonce>`), capped at 40,000 characters (cut at a paragraph end, with a log line), files at most 20 MB. Only `comment` edits are applied; replace/insert from the model are skipped with a log line. Formats: txt, md, docx, docm, odt, rtf, html (no PDF: there is no text extraction in the repo). The picker is an in-app list of the files in the allowed folders (depth 3, no links, no hidden files), not the native dialog, which cannot be limited to folders.
- **Rejected:** a per-assistant allow-list (one list is simpler to reason about); adding a PDF text-extraction dependency for this step; reading the file on a background thread (parsing a file of at most 20 MB on the UI thread is accepted for now).


## 2026-10-13 — A5 spike: how authorship survives Word and WordCraft

**Context.** HANDOFF §10 keeps AI authorship after a suggestion is accepted (V2–V4). It must be
stored in the `.docx` so Word and upstream WordCraft give the file back intact (losing marks is
tolerable, a refused or repaired file is not), invisible to a person editing in Word. Spike
report: `docs/authorship-storage.md`; harness: `scripts/authorship-spike/run.sh`.

**Decision.** **A: hidden bookmarks `_MarginAI_<id>` around the AI runs + one custom XML data
part** (`customXml/itemN.xml`, namespace `urn:margin:authorship`) with one entry per span:
author, model, acceptedAt, the accepted text (hash only above ~10k chars). Both pieces already
round-trip through the model (`InlineObject::BookmarkStart/End`, `Document::passthrough`), so
no docx-crate change here or upstream. On load Margin reconciles part and bookmarks (table in
the report §6); the part is rewritten from the model on save. Margin's editor inserts text typed
at a span's end *after* the bookmark end, matching Word's documented bookmark rule.

**Measured.** Margin and upstream WordCraft (`storytold/wordcraft` main `159b84f`), open+save and
save after edits inside / at the edges of the spans: A kept and tracking edits; B (offsets in the
part only) broken by the first insertion; C (`mc:Ignorable` attributes on runs) dropped on the
first save even without edits; D (hidden content controls) kept and tracking edits; E (hidden
character style) kept but leaks onto text typed after a span. All outputs valid (parts, content
types, relationships, bookmark ids).

**Not measured.** Word: not installed on this Mac (no bundle, no `com.microsoft.Word` in Launch
Services). The AppleScript round trip is written and wired in; run it before building A5's
persistence. If Word drops bookmarks in a flow that matters, D is the fallback.

**Alternatives rejected.** B: no anchor, cannot be repaired. C: dies in WordCraft today and
would need reader/writer work here and upstream, and Word is unlikely to keep it. D: Word's
right-click *Remove Content Control* and Developer tab expose it, and typing at the end of a
control stays inside it (a person's continuation becomes "AI"). E: no per-span identity, style
inheritance over-attributes. Comments (visible), unaccepted tracked changes (pending, not
accepted), permission ranges (Restrict Editing semantics), `docProps/custom.xml` (flat, shown
to the user, no anchor).

**Open.** Whether to store the accepted text in the part (precise repair after edits, file grows
by the AI share) or hash only: report recommends text with a cap; decide when A5 is built.

## 2026-10-13 — Assistant A5: authorship view and persistence (V1–V4)

**Context.** HANDOFF §10: keep "this text was written by the AI" after a suggestion is accepted,
show it (pill, dotted text, chips, status split) and store it in the `.docx`. The spike (ADR above)
chose candidate A; the owner decided to build V1–V4 and persistence now, on A, without a Word
measurement (Word is not installed on this Mac).

**Decisions.**
- **Storage A as built:** hidden `_MarginAI_<8 hex>` bookmarks around each accepted assistant
  insertion + one custom XML part `urn:margin:authorship` (one span per bookmark: author as
  written, model id, acceptedAt, the accepted text; hash only above 10,000 characters). The
  stored text is kept (not hash only): it lets a person's edits inside a span be attributed to the
  person by a bounded diff. Written only while there is authorship; a file without it is byte
  for byte as before. Report: `docs/authorship-storage.md` §8.
- **One matcher.** Who is an assistant is decided by `Session::is_assistant_author` over one
  list (`Session::assistant_authors`: configured name + every name used by `enter_assistant`) with
  one rule (`authorship::name_matches`); the UI's `review_panel::is_assistant` uses the same
  rule over the same list. The parallel fix branch (optional author name, `AI · <model>` names,
  a persisted set) only swaps those bodies. The author is stored as written on the change.
- **Word's bookmark rule for typing:** text typed, pasted or split with Enter at a span's end goes
  after the bookmark end (the person's); inside stays inside. Scoped to `_MarginAI_` bookmarks;
  ordinary bookmarks unchanged (`docs/typing-parity.md`, tests in `tests_typing.rs`).
- **Merging:** a new span touching or inside an *unedited* span of the same author extends it;
  an edited one is left alone (no re-attribution of a person's letters).
- **Pending suggestions are counted apart** (status bar "n % pending", pill note, never in chips).
  Chips from 10 % AI characters per paragraph; percentages whole, summing to 100, grouped by
  assistant name shown without an `AI · ` prefix.
- **Other formats drop authorship** (store and bookmarks), with a log line, `authorshipDropped`
  in the save result and a status message. Outside assistants may not create or remove marks
  (`insert.bookmark` with a `_MarginAI` name is denied).
- **Model id stored by default** (not a secret; empty for outside assistants).

**Alternatives rejected.** Hash-only entries (cannot attribute a person's edit inside a span);
authorship on runs or as a `Revision` field (would need docx reader/writer changes here and
upstream and would not survive Word); counting pending suggestions as AI (they are not accepted
text; the handoff's "cheaper first step" is kept as the separate pending share); leaving
bookmarks in HTML/Markdown exports (they turn into anchors named `_MarginAI_…`).

**Not measured.** Word round trips (`scripts/authorship-spike/run.sh`, Word stage skipped).
Upstream round trips of the feature's own document: kept.

## 2026-10-14 — Product name: Optimo (was Margin)

**Context.** Martijn renamed the app on 2026-10-14 ("Optimo", Sursilvan for "optimised") and
approved: rename everywhere, carry his settings and API key over automatically, new icon concept A.
The GitHub repo and the project folder are renamed separately (by the coordinator, last).

**Decision.**
- **Name:** `skin::APP_NAME = "Optimo"` (window title, eframe app name), macOS menus ("About / Hide /
  Quit Optimo", app menu "Optimo"), Settings › Assistant ("Let an assistant work in Optimo", "Allow
  assistants outside Optimo to connect", "Outside Optimo"), the assistant's system prompt
  ("inside Optimo, a word processor"), status and error messages, HTTP user agent `Optimo/<version>`,
  README, CLAUDE.md, docs. The i18n catalogs had no product-name strings (only "Left / Right / Top /
  Bottom Margin", which are page margins), so no catalog changed.
- **Bundle id** `ai.spert.optimo`; `packaging/macos/package.sh` now defaults to Optimo's branding
  (`Optimo.app`, `optimo.icns`, `optimo-<version>-macos-<arch>.dmg`); WordCraft's values are listed
  in its header for upstream builds.
- **Settings folder** `~/Library/Application Support/Optimo` (`control_key::APP_DIR`). On start,
  when `Optimo/ui.json` is missing, `ui.json` and `custom-dictionary.txt` are copied from the newest
  previous folder that has preferences: `Margin/`, else `WordCraft/` (`PREVIOUS_DIRS`,
  `migrate_from_previous`). Copied, never moved, never overwriting. Tests: one per hop
  (`hop_margin_to_optimo_is_preferred_and_keeps_the_old_folder`,
  `hop_wordcraft_to_optimo_when_there_is_no_margin_folder`) + `migrate_copies_once_and_never_overwrites`.
- **Persisted layout settings:** the `ui.json` key `margin` became `optimo` (`UiState::optimo`,
  `OptimoPrefs`); `UiState::from_json` moves an old `margin` object to `optimo` when `optimo` is
  missing (`margins_layout_settings_survive_the_rename`).
- **Keychain** service `ai.spert.optimo.assistant` (`keys::SERVICE`). On start, once (marker file
  `keychain-migrated` in the settings folder), each provider's key is copied from
  `ai.spert.margin.assistant` (`keys::LEGACY_SERVICE`) where Optimo has none; the old entries stay as
  a backup; a key removed later in Optimo is not brought back; if the keychain reports an error the
  marker isn't written and the copy is tried again at the next start. Only provider ids are logged,
  never keys. Test on memory stores only (`keys_move_from_margins_service_to_optimos_once_and_never_overwrite`).
  macOS may ask once whether Optimo may read the old "ai.spert.margin.assistant" items (they were
  created by Margin's binary).
- **Authorship format:** new marks `_OptimoAI_<8 hex>`, part namespace `urn:optimo:authorship`.
  Reading accepts Margin's `_MarginAI…` marks and the `urn:margin:authorship` part; on load
  `authorship::upgrade_legacy` renames marks and spans (`_MarginAI_1a2b3c4d` → `_OptimoAI_1a2b3c4d`),
  so the next save writes only the new names and a second save is byte-stable
  (`a_margin_file_keeps_its_spans_and_is_saved_in_optimos_format`, `margins_marks_are_renamed_to_optimos`).
  Outside assistants can't forge either prefix.
- **Scripts:** `scripts/optimo-run.sh` (the old `scripts/margin-run.sh` wrapper was removed 2026-10-19);
  `packaging/macos/optimo-icon.sh`; the authorship spike harness writes the new names.
- **Icon, concept A:** `assets/app-icon/optimo.svg` — Margin's blue tile (same shape, size, `#0A64D6`)
  with a white "O": left half a solid round-capped stroke, right half round dots ("solid you, dotted
  AI", the app's mark language). `optimo-small.svg` for 32 pt and below: thicker stroke, three bigger
  dots (checked at 16 and 32 px). Margin's icon files moved to `assets/app-icon/history/`.

**Renamed (code identifiers that mean the product):** `MarginPrefs` → `OptimoPrefs`, `MarginAct` →
`OptimoAct` (and the menu variant `Margin(..)` → `Optimo(..)`), `MarginTokens` → `OptimoTokens`,
`ui.margin` → `ui.optimo`, "Margin layout / skin / mode" in comments → "Optimo …". About 220
occurrences of the word "Margin" as the product name in code, scripts and living docs.

**Kept (page-layout "margin", or stable on purpose):** `egui::Margin` and `Anchor::Margin` (64),
the "Left / Right / Top / Bottom / Inside / Outside Margin" labels and their translations (77),
`MarginEdge` (33), "Margins", `margin_left` etc. (page geometry); the egui widget ids `"margin_*"`
(38; internal, never shown, renaming buys nothing); the Linux app id and
Windows/web icons (`ai.storyteller.wordcraft`, upstream's); dated decision records, the design
handoff text (`docs/design-handoff/**`, with a note on top of BRIEF.md and HANDOFF.md), STATUS.md and
IDEAS.md (owned by main), and the "Margin fork contributors" author name on ATTRIBUTION rows of
files made before the rename. README-WordCraft.md stays upstream's.

**Alternatives rejected.** Moving (not copying) the settings folder and keychain items: no way back
if Optimo misbehaves. Keeping the old file-format names: files would carry the old product name
forever. Serde aliases for `margin`: a JSON-level move keeps one key in the file.

## 2026-10-15 — Optimo O1: the bare window (OPTIMO.md §2)

**Context.** OPTIMO.md W1–W4: a fresh window shows only the traffic lights, the title, "Anything
⌘Y", Co-writer, the page and a bottom line. The panels open from ⌘Y, the menu bar or a shortcut
(Format ⌥⌘I, Review ⌥⌘V, Outline ⌥⌘O, "confirm free keys in the registry").

**Decision.**
- **Shortcuts (Martijn, 2026-10-15: "Format on F, Review on R, Outline on O", with Shift):**
  Format **⇧⌘F**, Review **⇧⌘R**, Outline **⇧⌘O**; ⌥⌘I stays as a second Format key (Pages).
  ⇧⌘F is Word's "font" key (`format.font`): in Optimo it opens Format on the Text tab with the
  font list open, so Word users still land on the font. ⌘D keeps the Font dialog. ⇧⌘R and ⇧⌘O
  were free. ⌥⌘F (Footnote), ⌥⌘O (Outline view), ⌥⌘V (Paste Special) and ⌃⌘F (macOS full
  screen) were taken; ⌥⌘R is reserved for Read as a reader (O6). The old Show Outline key ⌥⌘1
  clashed with Heading 1 and is gone. View menu gains "Show Review".
- **Panels are not remembered:** `inspector` is no longer saved in `ui.json`; every window starts
  bare. Ruler off by default in the Optimo layout (View › Show Ruler).
- **"Your words"** = people's characters / all characters from `Session::authorship`: pending
  suggestions *and* accepted AI text (A5's marks) count as not yours, so it is already better
  than OPTIMO.md's "pending only" first step. Click toggles View › Show Authorship.
- **W3 markers** replace the per-comment tag for the co-writer's open comments (grouped per
  paragraph, in the desk 20 pt right of the page); people's comments keep their R8 tag. A click
  opens Review on the first note until O3 brings margin notes.
- **⌘Y in O1:** the existing command field takes the pill's place while open; O2 moves it over
  the page.
- **Removed from the bottom line:** page count, language, tracking indicator, Pages/Pageless
  switch (all still in the menus); transient status messages stay.

**Alternatives rejected.** ⌥⌘V/⌥⌘O as specced (would shadow Paste Special and Outline view); ⌃⌘F/R/O (⌃⌘F is full screen);
keeping the inspector preference (an old default of "open" would show the panel on every
existing install).

## 2026-10-15 — Optimo O2: ⌘Y Anything (OPTIMO.md §3)

**Context.** Y1–Y2: one box for doing, finding, formatting and going to, replacing phase 6's
toolbar field (K) and the old Search Commands dialog.

**Decision.**
- **One box in both layouts.** ⌘Y, the pill, Help › Search Commands…, `ui.dialog commands`,
  Alt+Q (Windows/Linux) and the Classic ribbon's Commands button all open it; `Dialog::Commands`
  is gone. 640 wide, 92 below the window top, radius 12, shadow 0 12 40 at 16 %.
- **Repeat → ⇧⌘Y** (Martijn, 2026-10-15), added to Edit › Repeat; F4 stays. egui matches ⌘Y with
  an extra Shift, so ⌘Y is now consumed only on an exact match.
- **Empty state:** Format the selection chips (B, I, Heading 2, Quote, Link…, Comment, More
  format… → Format inspector) and Go to (headings; "Heading · empty" when no text follows before
  the next heading). The Co-writer group is not shown: its verbs land with O3–O9.
- **Typing:** matching commands (recent and frequent first), then matching headings, then
  matching recent files. A fixed group order, not one score (`ponytail:` note in `results`).
- **Ask rows hidden until O3** (§11). Today's ask lands as tracked suggestions, which rule 1
  replaces with margin notes. The three ⌘Y-ask tests are `#[ignore]`d with that reason, to be
  switched back on in O3. The context menu's "Ask …" opens the Co-writer panel instead.
- The floating mini toolbar hides while the box is open.

**Alternatives rejected.** Keeping the old dialog for Classic (two searches to maintain);
dropping Repeat's key entirely (Martijn chose ⇧⌘Y).


## 2026-10-19 — Optimo O3: co-writer engine (OPTIMO.md §0, §4)

**Context.** O3 is the co-writer's infrastructure: one verb runner, guards that enforce the five
product rules in code, Ask from ⌘Y, the margin-note component, presence, and the retirement of
phase 8's Assistant panel (Martijn, 2026-10-18: retire the panel, keep its settings; the rules
bind the in-app co-writer only, outside assistants over MCP keep their access). O4–O9 each add
one verb in parallel sessions, so the plug-in point must be small and stable.

**Decision.**
- **Runner in the engine** (`crates/engine/src/cowriter/`): `Verb` (the seven verbs + Ask),
  `Output` (Comment, Replacement, HeadingMove, HeadingInsert, Footnote, Quote, FirstLine,
  FormatChange), `prompt(verb, call)` → the model prompt (or `None` without a model),
  `run(verb, call, reply)` → guarded outputs, `apply(...)` → lands them through
  `run_as_assistant`. The network stays in the UI's background job (`crates/assistant`, Mistral
  via chat/completions); the engine never talks to a provider.
- **One file per verb, all present now.** `alternatives.rs`, `check.rs`, `format.rs`, `read.rs`,
  `research.rs`, `structure.rs`, `stuck.rs` are stubs (`AVAILABLE = false`, functions return
  "arrives in a later version"). A phase replaces its own file and sets `AVAILABLE = true`; no
  shared file changes (⌘Y lists `available()` verbs, the keys dispatch them, the runner calls the
  file). Options are a JSON object the verb reads itself. `format::COMMANDS` (the commands a
  format change may run) lives in the verb's file.
- **Guards** (`guard`, run by both `run` and `apply`): the §4 output table per verb; new words —
  none for Read/Check/Structure/Format, an alternative may not have more sentences than the
  selected text (and no more than 2× its words + 8), a first line is one sentence ≤ 40 words, a
  heading ≤ 12 words, a finding one sourced sentence ≤ 60 words (rule 4: title or address
  required); at most 50 outputs. Format for me runs inside `keeping_words`: the body's words
  before and after are compared after normalising typographic quotes, ellipses, dashes,
  whitespace and list markers at line starts; a difference restores the edit snapshot (document,
  selection, undo) and says how many words would have changed (rule 5). Every refusal is a line
  in the run's report; nothing refused changes the document.
- **Ask = notes only** (rule 1): a notes-only system prompt, at most 8 notes, the existing JSON
  edit protocol and its defensive reader; rewrites and insertions the model sends anyway become
  `Replacement`/`FirstLine` outputs that the guard refuses for Ask, so they are reported, not
  dropped silently. Notes land as comments by the AI author, kind "Note".
- **Shortcuts:** Alternatives ⌥⌘A; Read as a reader ⌥⌘R and Format for me ⌥⌘T on macOS only
  (Registered / Trademark sign elsewhere); Structure **⌃⌥⌘S** (⌥⌘S is Split, Word's key);
  Research **⇧⌥⌘F** (⌥⌘F is Footnote). A test keeps them clear of every registry command.
- **Margin notes** (`margin_notes.rs`): AI comments in the right desk margin, 268 wide, dotted
  rule, kind / text / Done · Remove, level with the anchor and pushed down on collisions; the
  last run's summary first; a 320 pt sheet when the desk is too narrow. In the Optimo layout the
  co-writer's notes never become balloons (they also no longer force the markup area open);
  people's comments keep balloons or tags. The Co-writer toggle and the quiet markers open the
  margin; a run that leaves notes opens it; Review stays closed. Opening the margin closes
  Review and Format (one thing on the right).
- **⌘Y:** the Co-writer group (landed verbs, then "Ask the co-writer…") above the chips; typed
  text gets "Ask the co-writer: “…”", first when nothing matches or it reads as a sentence. The
  scope chip now names what an ask is about: the selection, else "This section “…”" (the
  2026-10-13 incident's default), instead of "Whole document". Quick asks and the Ask mode (⇥)
  are gone: they asked for rewrites.
- **Retired:** the Assistant panel (composer, work log, suggestion rows, accept/reject from the
  panel, "Check facts against a file…" picker; `ui.assistant.suggestion` and `.facts`). Kept:
  Settings, now **Settings › Co-writer** ("Let a co-writer work in Optimo"), the provider/key/
  folders, `assistant_source` in the engine (O6's Check will use it), and Review's grouping of a
  multi-piece AI replacement (now fed by the co-writer's runs, for O5). New control command
  `ui.assistant.run {"verb": …}`; `ui.assistant.panel` toggles the margin view.
- **Presence:** "Co-writer is reading… · Stop" in the bottom line; the page caret stays.
- **Live check (Mistral Large 4, 2026-10-19):** with a 4096-token cap the reasoning model ran out
  before any text; Ask now uses `output_budget` (8–16k) like the old panel. Then: a one-line
  summary and notes on four quotes, no word of the document changed (`live-mistral-light.png`).
  The control port can't start an ask (it is an outside assistant); `ui_shot`'s
  `{"keychain": true}` runs one in-process.

**Alternatives rejected.** A trait object per verb registered in a list (one shared line per
phase, merge conflicts between parallel sessions); provider tool-calling for structured output
(differs per provider; the JSON protocol already survives Mistral's quirks); counting "new words"
by diffing against the document (an alternative legitimately reuses the person's words, so
sentences are the honest unit for rule 1); keeping the quick asks as notes (their names promise
rewrites); dropping the scope chip's selection wording (the chip is now the ask's scope, so it
must say the truth).

## 2026-10-19 — Optimo O4: Format for me (OPTIMO.md §5, T1–T7)

**Context.** The first verb on O3's runner. Mostly rule-based; the model only guesses the look.
Martijn's decisions: preset looks plus looks people save; Mistral is the model; no stop points.

**Decision.**
- **Rules** (`crates/engine/src/cowriter/format.rs`, `detect`): over the scope's top-level
  paragraphs (the selection, else the whole document — ⌥⌘T and ⌘Y turn a collapsed caret's
  "section" into the whole document). First short line (≤ 12 words, no closing `.,;!?`) → Title,
  a plain short line under it → Subtitle; short lines that are ALL-CAPS / bold / underlined and
  followed by text → Heading 1/2/3 by the strengths present (caps above bold above underline);
  `- * • –` and `1. 1) a)` lines → `para.bullets` / `para.numbering` (+ List Bullet / List Number
  styles), the typed marker removed; 2+ consecutive lines with the same columns (tabs, or gaps of
  2+ spaces) → a table in the look's table style with a header row; direct fonts → none (the
  look's fonts), monospace kept, colours kept unless Keep my colours is off; empty paragraphs
  between paragraphs → joined to the one before; `"` `'` → the language's quotes (« » for
  de-CH/fr/it-CH/rm, „ “ for de/cs/sk/pl, “ ” otherwise; ’ for apostrophes), `...` → `…`,
  ` - ` → ` – `, double spaces → one; title in the header and a page number in the footer
  (whole document, when it has neither). A doubled word or a would-be heading ending in a full
  stop is a "Check" note (a comment by the AI author on the exact quote), never a change: the
  coordinator allowed Comment outputs for Format for me (`Verb::allows`, the second mod.rs line).
- **Landing.** One output per rule, each one whitelisted command over a list of `targets`, run
  with `run_as_assistant` (tracked, AI author); all joined into **one undo step**
  (`join_next_undo`). Two new engine commands keep text changes inside rule 5:
  `format.typography` (character swaps; refuses any edit whose `same_words` differ, and joins a
  paragraph only to an *empty* next one, so words can't run together after accepting) and
  `format.clearFonts`. `text.insert`/`text.delete` stay off the whitelist.
- **Identity check** (coordinator-approved): `keeping_words` compares the
  body as it reads with every tracked change accepted (`final_body`: tracked deletions out), so a
  tracked typographic swap passes and a tracked word change still fails; someone's pending
  deletion from before the run counts the same before and after.
- **Looks** (`crates/engine/src/looks.rs`): Clean (Source Sans 3, teal headings), Classic
  (Source Serif 4, centred title), Report (sans headings in deep blue over serif text), Letter
  (quiet serif) — our colours, open fonts only. Each look sets the same fields of Normal, Title,
  Subtitle, Heading 1–3, List Bullet, List Number, Quote, Header, Footer (so switching leaves no
  trace of the last look) plus the "Look Table" table style, through `styles.modify` /
  `styles.create` / `table.newStyle` / `table.modifyStyle`, into the .docx style table. Switching
  (`{"look": …, "lookOnly": true}`) is one undo step and sends nothing.
- **The model** guesses the look in one call (≤ 80 tokens, first 3 000 characters, document
  marked untrusted): `{"look", "reads_like"}`. Without a model, with the co-writer off, or when
  the look is given, nothing is sent and the look is Clean (or the one given); a failed call
  formats with Clean and says so.
- **Saved looks:** "Save as my look…" stores `Look::from_doc` (each style as it resolves) in
  `looks.json` in the settings folder next to `ui.json` (the desktop app sets the path; the web
  build keeps them for the session). Read once, ≤ 256 KB, ≤ 24 looks, each made sane (known styles
  only, name ≤ 40 characters, sizes and spacing clamped); written via a temporary file.
- **View** (`crates/ui-egui/src/format_view.rs`): right margin — "Formatted by your co-writer",
  the lock row "Your words: 0 changed · N kept", the Look switcher and saved looks, What changed
  with Show (selects the first spot, flashes them all), the three options, Undo all
  · One by one (opens Review) as links and Keep as the button (three buttons don't fit 256 pt).
  The co-writer's formatting changes no longer get balloons in the Optimo layout (like its notes):
  the view sums them up and Review lists them. Left desk margin — the 264 wide Before thumbnail of the page
  in view and "hold ⌥ to compare" (⌥ lays pictures of the old pages over the pages). The view
  counts its undo steps (run, look switches, options, table of contents); Undo all undoes exactly
  those, and refuses once the person has edited since. Control: `ui.assistant.formatView`.
- **Table of contents** (T6, off): inserted after the identity check as its own step, before the
  first heading — it repeats heading text, so it would otherwise fail rule 5.

**Known limits.** Text → table conversion is not a tracked change in the engine (it is an undo
step, and Undo all covers it). The converted table keeps the borders `table.fromText` gives
it, so the Look Table's colour and bold header row don't show yet. The live Mistral look guess
was not exercised from the build session: the control channel is an outside assistant and may not
start the co-writer (O3), so it needs a person's ⌥⌘T; the guess is tested against a mock provider. The Look switcher's style-table changes are undo steps, not tracked
changes (as in Word).

**Alternatives rejected.** Whitelisting `text.insert` for typography (any text could pass the
command guard; only the end-of-run check would stop it); one output per change (the runner caps a
run at 50 outputs); a model pass for headings (the rules are deterministic and free); saving looks
inside `ui.json` (a broken look file must never cost the person their window settings).

## 2026-10-19 — Optimo O5: Alternatives (OPTIMO.md §6, A1–A3)

**Context.** Phase O5: three other ways to say the selected sentence, shown in a card under it;
using one is one tracked replacement by the AI author (rules 1–3).

**Decision.**
- **Engine** (`cowriter/alternatives.rs`, `mod.rs` untouched): the model answers in the existing
  edit protocol — three "replace" edits quoting the whole selection — so the defensive answer
  reader is shared. Duplicates and copies of the original are dropped; anything that is not a
  replacement of the selection (comments, insertions, other quotes) goes on to the guards and is
  refused there. One paragraph only ("Select sentences within one paragraph"). Options:
  `tone` (plainer, warmer, more formal, shorter) and `moreLike` (⇥). Landing is
  `assistant_edits::apply`, so only the changed words become tracked changes.
- **Card** (`alternatives_view.rs`): opens at once with "Looking for other ways…", under the
  selection at page width, radius 10; rows Yours / Sharper / Plainer / Warmer (labels by position;
  "Like it" after ⇥); changed words in the assistant colour with a dotted underline. Keys 1–3, ↑↓,
  ⏎, ⇥, E (edit), Esc; typing or moving the caret closes it. An edited text lands through the same
  guards. ⌥⌘A and ⌘Y both go through `cowriter::start` (no selection: `select.sentence` first).
  `ui.assistant.alternatives {"pick"|"text"|"close"}` drives the open card (the person's, not
  an outside assistant's).
- **Live check** (Mistral Large, `mistral-large-latest`, key from the Keychain via `ui_shot`'s
  `{"keychain": true}`): three alternatives in about 10 s; picking #2 left `Your words 56 %` and a
  word-level tracked change by "AI · Mistral" (`picked-light.png`). The model rewrites more than the
  prompt's "keep their words" asks; the card shows every changed word honestly.

**Known gaps.** The rows use the interface font, not the document's font: document fonts are not
egui font families (a face registered in `theme.rs` would fix it). The ✎ glyph is not in the
interface font, so the footer reads "E edit before using". Streaming is not used (one call).

**Alternatives rejected.** A new answer format for the verb (the edit protocol's reader already
survives Mistral's quirks); landing all three as separate tracked changes to choose later
(Review would hold three competing replacements of one sentence); labels carried by the model
(the runner's `Output` has no label, and `mod.rs` stays as it is).

## 2026-10-19 — Optimo O9: I'm stuck (OPTIMO.md §10, U1–U3)

**Context.** The last small verb on O3's runner. The person is at an empty paragraph; the
co-writer may ask, never write — except one opt-in line (rule 1's single exception).
Martijn's decisions: Mistral is the model; no stop points.

**Decision.**
- **Engine** (`crates/engine/src/cowriter/stuck.rs` + new `crates/engine/src/stuck.rs`; `mod.rs`
  untouched). One model call returns `{"report", "questions": [...], "first_line"}`. `read` keeps
  only items that are one line ending in `?` (≤ 40 words; prose is skipped with a line), at most 5,
  as `Comment` outputs of kind "Question"; `first_line` goes in raw as a `FirstLine` so the guard
  in `mod.rs` refuses a second line, a second sentence or more than 40 words. Anything that isn't
  JSON is an error and changes nothing. The first line is **held back**: the questions are not
  landed by `apply` but kept in the window (`stuck_view`); each is pinned only when clicked.
- **Pinning at the caret.** `stuck::pin` selects the collapsed caret in the empty paragraph and
  runs `review.newComment` as the AI author (a comment on an empty point, shown in the margin
  like any note); with comments by assistants off in Settings it says so and does nothing.
  Comment marks don't make the paragraph non-empty (`stuck::is_blank`).
- **First line** (U3). The click calls `cowriter::apply(Verb::Stuck, FirstLine)`: one tracked
  insert at the caret by the AI author, only while the caret still rests in the empty paragraph.
- **Entry for O7.** `stuck_view::enter(app, Some(paragraph path))` (or `None` for the caret's
  section): the caret goes into that paragraph if it is blank, else a new blank one follows it
  (as the person's own Enter), then the run starts. The same checks as ⌘Y apply (co-writer on, provider).
- **Placeholder.** Painted over the page at the caret (grey italic, not document text); the view
  ends when the paragraph gets any text or the caret leaves it (`stuck_view::tick`), so it goes with
  the first keystroke. Unpinned questions go with it; pinned ones stay as notes.
- **U2 "From your notes"** is rule-based and local (nothing extra is sent): other people's
  comments in this document that share a word (4+ letters) with the section, and, with folders
  allowed, the first matching line of up to 3 files in them whose *name* shares such a word.
  Files are matched by name only so a run never reads a whole folder.
- **Control hook.** `ui.assistant.stuckView {"answer": "<json>", "pin": i, "firstLine": true}`
  feeds a model answer through the real reader and guards without a provider (tests, screenshots).

**Alternatives rejected.** Landing all questions as comments at once (the person asked for a
quiet start); a second model call for the first line (cost, and a second wait); anchoring
questions on the heading (the note belongs where the person is writing); reading every linked
file for U2.

## 2026-10-20 — Optimo O6: Read as a reader and Check facts (OPTIMO.md §7, R1–R4)

**Context.** Phase O6: two notes-only verbs on O3's runner. Read as a reader leaves notes where
a chosen reader would stumble; Check facts and names compares numbers, dates and names with the
document itself and the person's allowed files, and offers a one-click fix that lands only when
clicked. `mod.rs` untouched.

**Decision.**
- **One answer format.** Both verbs use the edit protocol of `assistant_edits` (one JSON object,
  the shared defensive reader). Read's kind travels in the note: `"note": "Question: …"` →
  kind "Question" (also Unclear, Rhythm, Strong line; anything else is "Note"). Check's kind is
  always "Check"; a finding may carry `"with"`, the quote as it reads corrected. When quote and
  `with` differ in exactly one place (`word_diff`), the note gets "Use <new>" · "Keep <old>"
  (OPTIMO's "Use 34" / "Keep thirty"); otherwise it is a doubt with Done · Remove.
- **Guards.** Rewrites and insertions the model sends anyway go on to the runner's guards as
  what they are and are refused with a line; a note whose quote is not in the text read is
  refused in the verb ("it is not in the text that was read/checked"). Read keeps at most one
  note per paragraph and 8 per run (the mentor is selective); Check at most 12.
- **"Use …"** (`reader_view::choose`): one tracked replacement through `assistant_edits::apply`
  under the note's own AI author, word-level, so only the number or name is the AI's; then the
  note is done. "Keep …" only resolves the note. The options live in the UI per (document, note
  text) for the session: the runner lands comments without their options, and `mod.rs` stays.
- **Files.** Check reads only folders allowed in Settings › Co-writer; `cowriter::start` puts
  `allowed_folders()` in the options and overwrites anything a caller passed. It opens up to 12
  listed files, keeps the 4 that share the most of the document's numbers and capitalised words,
  within `MAX_SOURCE_CHARS`, each as its own `BEGIN SOURCE <nonce> <name>` block marked untrusted.
- **Reader chooser** (`reader_view.rs`): ⌥⌘R or ⌘Y without a reader opens "Read as" + a popup
  (A first-time reader, A busy manager, An expert, A sceptic, A funder, Custom…) at the top of the
  margin with Read · Not now; nothing is sent until Read. The last reader is kept per document for
  the session. After a reading the header keeps the popup and shows "4 notes · 1 question ·
  1 unclear · 1 rhythm · 1 strong line" and the report. `ui.assistant.run {"verb": "read",
  "reader": …}` runs at once; `{"open": true}` opens the popup.
- **Note links by kind:** Question/Unclear/Rhythm → Reply (a reply field under the note) ·
  Alternatives (O5 on that sentence) · Done; Strong line → Thanks; Check → Use … · Keep … (or
  Done · Remove); other notes keep Done · Remove.
- **Shared-file hooks:** `cowriter.rs` (`reader` field; `before_start` at the top of `start`;
  `remember` before `apply`), `margin_notes.rs` (the Read header branch, links by kind, the reply
  field), `lib.rs` (`pub mod reader_view`), one test appended to `cowriter/tests.rs`.

**Known limits.** The live Mistral run was not done: the Keychain did not hand the key to the
newly built `ui_shot` within 30 s (a macOS prompt for the new binary); the screenshots use a
local scripted provider (`phase-O6/*.png`), and the mock-provider test covers the full path. A
quote that runs across another note's anchor is skipped by `locate` ("runs across a tracked
change or an object"), so a Check after a Read can lose a finding on the same words. The last
reader is kept for the session, not saved in the file. Check opens the files on the UI thread
(12 files at most).

**Alternatives rejected.** A label field per note (the shared reader drops unknown keys; the
prefix travels inside the note text and survives repairs); anchoring Check notes on the bare
number (often not unique, so `locate` refuses it); hidden encodings in option strings (the
visible "Use"/"Keep" labels carry exactly what the fix needs); running Read straight away with
the last reader (OPTIMO R1 asks who the reader is first).
