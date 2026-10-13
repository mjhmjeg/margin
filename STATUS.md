# STATUS — WordCraft for Mac (fork of storytold/wordcraft)

Last updated: 2026-10-13 (afternoon)

Repo: https://github.com/mjhmjeg/margin (fork of storytold/wordcraft; remote `upstream`).

Goal: a Mac-native-feeling UI (slim toolbar, right format inspector, real menu bar) on top of the
WordCraft engine. Public on GitHub, offered back upstream as PRs. Plan: see docs/decisions.md.

## Now
- Design fine-tuning in Claude Design (Martijn) — handoff in docs/design-handoff/ (BRIEF.md + 22 screens), zip in dist/ ✋
- Phase 5: packaging works (Margin.app, ai.spert.margin, icon, README; package.sh takes APP_NAME/BUNDLE_ID/APP_ICNS/FILE_PREFIX/NOTARY_PROFILE). Waiting on Martijn: Developer ID identity name + notarytool keychain profile → signed universal build → draft GitHub release ✋

## Next
- Review branch design/phases-3-5 in the running app (scripts/margin-run.sh), then merge to main / open the upstream PRs (Opus, high)
- Design phase 6 (command field K1) and phase 7 (Review panel, R9 status, Assistant toggle) per HANDOFF.md (Sonnet, high)

## Later
- AI co-worker (see IDEAS.md): interview + plan first; design exploration now in Claude Design (docs/design-handoff/REVIEW-AND-AI.md)
- Weekly upstream merge (scheduled, Sonnet)

## Done
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
- Handoff icon set (§11) not ported; the Link icon reads like an eye at 17 pt.
- Margin shares the settings folder (~/Library/Application Support/WordCraft) with an installed WordCraft.
- Upstream warning: contributors/people.toml TOML parse error at line 71 (theirs).
- Toolbar sits in the 28 pt traffic-light band (AppKit fixes the lights; moving them needs unsafe code, forbidden upstream) — slimmer than the 52 pt mockup.
- Pageless and the inspector settings are app-wide, not saved per document.
- 2026-10-12 Incident (twice): app binary not rebuilt after code changes, Martijn saw the old app. Fix: always launch via scripts/margin-run.sh (rebuilds, refuses stale binary); verify the live window via the control channel before reporting.
- /Applications/WordCraft.app (upstream) is installed too; opening it from Dock/Spotlight shows upstream, not Margin.
