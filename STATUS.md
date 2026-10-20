# STATUS — Optimo (was Margin; fork of storytold/wordcraft)

Last updated: 2026-10-19

Repo: https://github.com/mjhmjeg/margin (fork of storytold/wordcraft; remote `upstream`).

Goal: a Mac-native-feeling UI (slim toolbar, right format inspector, real menu bar) on top of the
WordCraft engine. Public on GitHub, offered back upstream as PRs. Plan: see docs/decisions.md.

## Now
- Design fine-tuning in Claude Design (Martijn) — handoff in docs/design-handoff/ (BRIEF.md + 22 screens), zip in dist/ ✋
- Phase 5: packaging works (Margin.app, ai.spert.margin, icon, README; package.sh takes APP_NAME/BUNDLE_ID/APP_ICNS/FILE_PREFIX/NOTARY_PROFILE). Waiting on Martijn: Developer ID identity name + notarytool keychain profile → signed universal build → draft GitHub release ✋

## Next
- Live Mistral pass in the app over Read as a reader, Check facts, I'm stuck and Research (Martijn allows the Keychain prompt once; Sonnet, medium) ✋
- Polish: selection toolbar overlaps the heading above the Alternatives card; a person's own comment shows as a Word-style balloon clipped beside the co-writer margin (Sonnet, medium)
- Martijn: try ⌥⌘T Format for me on a messy document (the live look guess is untested) ✋
- Martijn: try ⌘Y in the running app (scripts/optimo-run.sh on branch rename/optimo) ✋
- Martijn: try branch assistant/integrate in the running app (Mistral key, ⌘Y on a selection, Show Authorship, Home › Back); clear the Author name field to get "AI · <model>" ✋
- Run scripts/authorship-spike/run.sh on a Mac with Word (Word not installed here) ✋
- Merge assistant/integrate to main, then upstream PRs per phase (Opus, high)
- Translations for the new assistant/Home strings (18 catalogs) (Sonnet, low)

## Later
- Weekly upstream merge (scheduled, Sonnet)

## Done
- 2026-10-20 Optimo O8: Research (ask in the margin, findings each with a source from your files or Mistral's own web search, store off; unsourced findings refused; Add as footnote / quote = one tracked change with the source); merged on rename/optimo, xtask ci green. Not built: "Claims in this document" list (Check facts covers it), sources list at the end (each footnote carries its source). All Optimo phases O0–O9 are now on rename/optimo
- 2026-10-20 Optimo O7: Structure (outline in the left margin with section lengths, suggested heading moves and missing headings landing as tracked changes on accept, empty sections link to I'm stuck, impossible suggestions refused and reported; live Mistral checked); merged on rename/optimo, xtask ci green
- 2026-10-20 Optimo O6: Read as a reader (reader chooser, notes where that reader stumbles, kinds Question/Unclear/Strong line) and Check facts and names (numbers, dates, names against the document and allowed folders; Check notes with "Use 12 / Keep 11" options landing as one tracked change on click); merged on rename/optimo, xtask ci green. Live Mistral run not done (Keychain prompt)
- 2026-10-20 Optimo O9: I'm stuck (grey placeholder in the empty paragraph, 3–5 questions in the margin pinned on click, "From your notes" from earlier comments, one opt-in rough first line as a tracked insert); merged on rename/optimo, xtask ci green. Live Mistral run not done yet. Polish: a person's own comment still shows as a Word-style balloon clipped beside the co-writer margin
- 2026-10-19 Optimo O5: Alternatives (card under the sentence: Yours / Sharper / Plainer / Warmer, tone popup, changed words marked; picking one = one tracked replacement by the AI; live Mistral checked); plus keychain read moved off the UI thread (window no longer freezes behind the macOS prompt; not yet tried with a real prompt). Merged on rename/optimo, xtask ci green. Polish: the selection toolbar overlaps the heading above the card
- 2026-10-19 Optimo O4: Format for me (rule-based headings, lists, tables, fonts, spacing, typography; Clean/Classic/Report/Letter looks + "Save as my look"; Before thumbnail, ⌥ compare; margin view with "Your words: 0 changed"; Check notes for word-level findings); merged on rename/optimo, xtask ci green. Limits: text→table is an undo step, not a tracked change; live look guess not yet run
- 2026-10-19 Optimo O3: co-writer engine (verb runner + guards for the five rules in crates/engine/src/cowriter/, notes in the right margin, ⌘Y Co-writer group + Ask row, "Co-writer is reading… · Stop", old Assistant panel retired, Settings › Co-writer); live Mistral ask landed notes only; merged on rename/optimo, xtask ci green
- 2026-10-15 Optimo O2: ⌘Y Anything box (scope chip, Format chips, Go to headings, commands + headings + recent files); old Search Commands dialog removed (both layouts); Repeat → ⇧⌘Y; ask row hidden until O3; on rename/optimo, xtask ci green, screens docs/design-handoff/phase-O2/
- 2026-10-15 Optimo O1: bare window (centred title, Anything ⌘Y pill, Co-writer, bottom line "words · Your words %", quiet "2 notes" markers, ruler off); panels on ⇧⌘F Format (opens on the font list) / ⇧⌘R Review / ⇧⌘O Outline; on branch rename/optimo, not pushed, xtask ci green, screens docs/design-handoff/phase-O1/
- 2026-10-14 Optimo O0: rename Margin → Optimo (bundle ai.spert.optimo, settings + Keychain + authorship marks carried over, icon concept A); on branch rename/optimo
- 2026-10-14 A5 authorship view + .docx persistence (hidden bookmarks + custom XML part; WordCraft round trip ok); assistant fixes: cut-off/JSON answers, ⌘Y uses selection / This section, word-level suggestions, "AI · <model>" author, Home Back; branch assistant/integrate pushed, xtask ci green
- 2026-10-13 A2 providers + Keychain key (S1–S3), A3 Assistant panel + Ask from ⌘Y (A1–A5, K3–K4), A4 read-only folders + fact-check against a file (S5, K4); all pushed, xtask ci green; mock provider only, no live key tested
- 2026-10-13 Icons (§11, classic ribbon keeps old ones), Review panel (phase 7), command field ⌘Y/⌥Q (phase 6), A1 outside assistants as tracked suggestions + Settings › Assistant; branch assistant/outside pushed, xtask ci green
- 2026-10-13 Assistant interview + plan (docs/decisions.md)
- 2026-10-13 Design phases 3–5 (editor, Home, dialogs incl. all dialog dropdowns + number fields) merged on branch design/phases-3-5; Insert Table hover grid wired; `cargo xtask ci` all 6 steps green; live window checked
- 2026-10-13 Pages-style Home (templates, Recent thumbnails, File › New); card polish; design handoff
- 2026-10-12 Phase 4: native menu bar (9 menus, 67 items); fix Export as PDF (save dialog)
- 2026-10-12 Phase 3: Mac layout (toolbar, Format inspector, outline, Pageless, Classic ribbon)
- 2026-10-12 Phase 2: Margin skin (Mac palette, Apple blue, 13 pt), follows macOS light/dark
- 2026-10-12 Phase 1: mockup approved (flat, Apple blue, pageless, one type size) — docs/mac/mockup.html
- 2026-10-12 Phase 0: fork builds and runs on macOS; baseline shots docs/mac/before/
- 2026-10-12 Research storytold org; chose WordCraft, option B (restructure UI on egui)

## Open decisions
- OPTIMO.md §13: (1) ⌘Y Repeat → ⇧⌘Y — decided 2026-10-15 (2) Look list Clean/Classic/Report/Letter, user-saved looks? (3) Research: provider web search or own search API key?
- Branch rename/optimo (O0–O9 complete, built on assistant/integrate) not merged to main or pushed yet.
- Product name: Optimo (2026-10-14, replaces Margin). No formal trademark search done.

## Known bugs
- 2026-10-19 Incident: disk full (117 MB free), project build cache had grown to 56 GB; target/debug (52 GB) deleted, 52 GB free. Parallel sessions must keep their own target dirs small and delete them when done.
- Adding a comment opens the Review panel (engine behaviour); for co-writer notes that defeats O1's quiet markers — fix in O3.
- 2026-10-14 Incident: upstream WordCraft (in /Applications) rewrote the shared ui.json and wiped Margin's assistant settings (key in Keychain unaffected). Fixed: Margin uses Application Support/Margin, copies the old file once (test + verified on this Mac).
- 2026-10-13 Incident: whole-document ask with Mistral failed ("no JSON object"): 4096-token cap cut the answer; Mistral reasoning chunks dropped. Fixed with tests (salvage, JSON mode, budget 8–16k). ⌘Y ask ignored the selection (⇥ moved focus) — fixed with key-event tests.
- 2026-10-13 Incident: disk full twice from two parallel debug builds (~50 GB). Rule now: one build at a time.
- Review groups word-level pieces only for the session; after restart they show as separate rows.
- Assistant work log is per session only (not saved with the document).
- Authorship: Word round trip unmeasured (Word not installed).
- Fact-check reads txt/md/docx/odt/rtf/html; no PDF (no text extraction in the repo).
- Outside assistants can no longer read any file unless a folder is allowed in Settings (A4 behaviour change).
- Upstream floating format bar can overlap a heading over a selection (handoff dropped it).
- 2026-10-13 Incident: disk full stopped two design helpers mid-build. Cause: each worktree had its own target dir (up to 22 GB each). Fix: worktrees removed after merge; one shared target only, and with a shared target `touch` sources before CI (cargo reused a binary built from another worktree).
- E19: switching Page numbers off only shows a message — no engine command removes page numbers.
- Review "Show": Simple markup / Original draw like All / No markup (engine has markup on/off only).
- Headless `wordcraft-cli mcp` edits files directly, not as tracked suggestions.
- Upstream warning: contributors/people.toml TOML parse error at line 71 (theirs).
- Toolbar sits in the 28 pt traffic-light band (AppKit fixes the lights; moving them needs unsafe code, forbidden upstream) — slimmer than the 52 pt mockup.
- Pageless and the inspector settings are app-wide, not saved per document.
- 2026-10-12 Incident (twice): app binary not rebuilt after code changes, Martijn saw the old app. Fix: always launch via scripts/optimo-run.sh (rebuilds, refuses stale binary); verify the live window via the control channel before reporting.
