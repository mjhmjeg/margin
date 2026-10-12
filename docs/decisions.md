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
