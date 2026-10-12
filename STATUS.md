# STATUS — WordCraft for Mac (fork of storytold/wordcraft)

Last updated: 2026-10-12

Repo: https://github.com/mjhmjeg/margin (fork of storytold/wordcraft; remote `upstream`).

Goal: a Mac-native-feeling UI (slim toolbar, right format inspector, real menu bar) on top of the
WordCraft engine. Public on GitHub, offered back upstream as PRs. Plan: see docs/decisions.md.

## Now
- Phase 2: visual pass done (skin.rs: Mac palette, Apple blue, 13 pt / 400 everywhere except Classic ribbon; no Discord/avatar/AutoSave) — awaiting Martijn check in the app ✋ (launch: scripts/margin-run.sh)

## Next
- Phase 3: toolbar + format inspector + outline sidebar + Pageless (view.webLayout); ribbon behind View › Classic (Opus, high) ✋

## Later
- Phase 4: native macOS menu bar mapped to engine commands (Sonnet, high)
- Phase 5: signed + notarised .app, GitHub release, README (Sonnet, medium) ✋
- Weekly upstream merge (scheduled, Sonnet)

## Done
- 2026-10-12 Phase 1: mockup approved (flat, Apple blue, pageless, one type size) — docs/mac/mockup.html
- 2026-10-12 Phase 0: fork builds and runs on macOS; baseline shots docs/mac/before/
- 2026-10-12 Research storytold org; chose WordCraft, option B (restructure UI on egui)

## Open decisions
- Product name: "Margin" chosen 2026-10-12. Near-clash: an early Mac markdown notes app "Margin" (peerlist) and iPad tracker "The Margin". No formal trademark search done.

## Known bugs
- 2026-10-12 Incident (twice): app binary not rebuilt after code changes, Martijn saw the old app. Fix: always launch via scripts/margin-run.sh (rebuilds, refuses stale binary); verify the live window via the control channel before reporting.
- /Applications/WordCraft.app (upstream) is installed too; opening it from Dock/Spotlight shows upstream, not Margin.
