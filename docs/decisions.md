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
