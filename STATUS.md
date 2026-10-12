# STATUS — WordCraft for Mac (fork of storytold/wordcraft)

Last updated: 2026-10-12

Repo: https://github.com/mjhmjeg/margin (fork of storytold/wordcraft; remote `upstream`).

Goal: a Mac-native-feeling UI (slim toolbar, right format inspector, real menu bar) on top of the
WordCraft engine. Public on GitHub, offered back upstream as PRs. Plan: see docs/decisions.md.

## Now
- Phase 4: native macOS menu bar done (mac_menu.rs data + apps/wordcraft/src/mac_menu.rs via muda =0.21.2; 9 menus, 67 items) — awaiting Martijn check: shortcuts ⌘C/⌘V/⌘Z/⌘B in page and text fields ✋

## Next

## Later
- Phase 5: signed + notarised .app, GitHub release, README (Sonnet, medium) ✋
- Weekly upstream merge (scheduled, Sonnet)

## Done
- 2026-10-12 Phase 3: Mac layout (toolbar, Format inspector, outline, Pageless, Classic ribbon)
- 2026-10-12 Phase 2: Margin skin (Mac palette, Apple blue, 13 pt), follows macOS light/dark
- 2026-10-12 Phase 1: mockup approved (flat, Apple blue, pageless, one type size) — docs/mac/mockup.html
- 2026-10-12 Phase 0: fork builds and runs on macOS; baseline shots docs/mac/before/
- 2026-10-12 Research storytold org; chose WordCraft, option B (restructure UI on egui)

## Open decisions
- Product name: "Margin" chosen 2026-10-12. Near-clash: an early Mac markdown notes app "Margin" (peerlist) and iPad tracker "The Margin". No formal trademark search done.

## Known bugs
- App menu shows the process name ("wordcraft") until phase 5 bundles it as Margin.app.
- Pageless: faint page-shadow lines left and right of the column (canvas draws the page edge).
- Toolbar sits in the 28 pt traffic-light band (AppKit fixes the lights; moving them needs unsafe code, forbidden upstream) — slimmer than the 52 pt mockup.
- Pageless and the inspector settings are app-wide, not saved per document.
- 2026-10-12 Incident (twice): app binary not rebuilt after code changes, Martijn saw the old app. Fix: always launch via scripts/margin-run.sh (rebuilds, refuses stale binary); verify the live window via the control channel before reporting.
- /Applications/WordCraft.app (upstream) is installed too; opening it from Dock/Spotlight shows upstream, not Margin.
