# STATUS — WordCraft for Mac (fork of storytold/wordcraft)

Last updated: 2026-10-13 (afternoon)

Repo: https://github.com/mjhmjeg/margin (fork of storytold/wordcraft; remote `upstream`).

Goal: a Mac-native-feeling UI (slim toolbar, right format inspector, real menu bar) on top of the
WordCraft engine. Public on GitHub, offered back upstream as PRs. Plan: see docs/decisions.md.

## Now
- Design fine-tuning in Claude Design (Martijn) — handoff in docs/design-handoff/ (BRIEF.md + 22 screens), zip in dist/ ✋
- Phase 5: packaging works (Margin.app, ai.spert.margin, icon, README; package.sh takes APP_NAME/BUNDLE_ID/APP_ICNS/FILE_PREFIX/NOTARY_PROFILE). Waiting on Martijn: Developer ID identity name + notarytool keychain profile → signed universal build → draft GitHub release ✋

## Next
- A2 providers + key: Settings switch (default off), Anthropic / OpenAI / Gemini / Mistral (pre-selected) / OpenRouter / On this Mac / Other, Keychain key, live model list (Opus, high) — running
- A3 Assistant panel + "Ask Claude" in ⌘Y (Opus, high); A4 folder access (Sonnet, medium); A5 authorship view — pause for Martijn first (Fable, xhigh)
- Then: merge the chain to main, upstream PRs per phase (Opus, high)

## Later
- Weekly upstream merge (scheduled, Sonnet)

## Done
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
- Product name: "Margin" chosen 2026-10-12. Near-clash: an early Mac markdown notes app "Margin" (peerlist) and iPad tracker "The Margin". No formal trademark search done.

## Known bugs
- 2026-10-13 Incident: disk full stopped two design helpers mid-build. Cause: each worktree had its own target dir (up to 22 GB each). Fix: worktrees removed after merge; one shared target only, and with a shared target `touch` sources before CI (cargo reused a binary built from another worktree).
- E19: switching Page numbers off only shows a message — no engine command removes page numbers.
- Review "Show": Simple markup / Original draw like All / No markup (engine has markup on/off only).
- Headless `wordcraft-cli mcp` edits files directly, not as tracked suggestions.
- Margin shares the settings folder (~/Library/Application Support/WordCraft) with an installed WordCraft.
- Upstream warning: contributors/people.toml TOML parse error at line 71 (theirs).
- Toolbar sits in the 28 pt traffic-light band (AppKit fixes the lights; moving them needs unsafe code, forbidden upstream) — slimmer than the 52 pt mockup.
- Pageless and the inspector settings are app-wide, not saved per document.
- 2026-10-12 Incident (twice): app binary not rebuilt after code changes, Martijn saw the old app. Fix: always launch via scripts/margin-run.sh (rebuilds, refuses stale binary); verify the live window via the control channel before reporting.
- /Applications/WordCraft.app (upstream) is installed too; opening it from Dock/Spotlight shows upstream, not Margin.
