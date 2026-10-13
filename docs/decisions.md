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

