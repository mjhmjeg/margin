# STATUS — WordCraft for Mac (fork of storytold/wordcraft)

Last updated: 2026-10-12

Repo: https://github.com/mjhmjeg/margin (fork of storytold/wordcraft; remote `upstream`).

Goal: a Mac-native-feeling UI (slim toolbar, right format inspector, real menu bar) on top of the
WordCraft engine. Public on GitHub, offered back upstream as PRs. Plan: see docs/decisions.md.

## Now
- Phase 0: built + running on macOS (2m15s release build); baseline shots in docs/mac/before/ — done 2026-10-12

## Next
- Phase 1: mockup at docs/mac/mockup.html — awaiting Martijn approval ✋ (Opus, high)
- Phase 2: visual pass — tokens, Inter, spacing, title bar; drop Discord/avatar/AutoSave (Sonnet, medium) ✋
- Phase 3: toolbar + format inspector + outline sidebar + Pageless (view.webLayout); ribbon behind View › Classic (Opus, high) ✋

## Later
- Phase 4: native macOS menu bar mapped to engine commands (Sonnet, high)
- Phase 5: signed + notarised .app, GitHub release, README (Sonnet, medium) ✋
- Weekly upstream merge (scheduled, Sonnet)

## Done
- 2026-10-12 Research storytold org; chose WordCraft, option B (restructure UI on egui)

## Open decisions
- Product name: "Margin" chosen 2026-10-12. Near-clash: an early Mac markdown notes app "Margin" (peerlist) and iPad tracker "The Margin". No formal trademark search done.

## Known bugs
- none yet
