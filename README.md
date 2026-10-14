<p align="center"><img src="assets/app-icon/optimo-1024.png" width="128" alt="Optimo app icon: a white O on a blue tile, its left half solid and its right half dotted"></p>

<h1 align="center">Optimo</h1>

<p align="center"><i>Formerly Margin (renamed 14 October 2026). “Optimo” is Sursilvan for “optimised”.</i></p>

<p align="center"><b>A Mac-native interface for <a href="https://github.com/storytold/wordcraft">WordCraft</a>, the open-source word processor in pure Rust.</b></p>

<p align="center"><img src="docs/mac/hero.png" alt="Optimo with the Format inspector open on the right and the Open Studio Handbook sample document" width="100%"></p>

Optimo keeps WordCraft's document engine (.docx in and out, styles, tables, track changes,
references, PDF export) and replaces its Office-style ribbon with an interface that feels at home
on macOS:

- **One slim toolbar** instead of the ribbon: paragraph style, table, image, shape, comment, track changes, zoom.
- **A Format inspector** on the right, like Pages: Text, Layout and Document tabs.
- **The real macOS menu bar**, with the shortcuts you expect.
- **Pageless mode**, like Google Docs: one continuous sheet, Narrow / Medium / Full text width. Printing and PDF export still use pages.
- **Follows your Mac's light or dark appearance**, Apple's system blue, one calm type size throughout.
- **The classic ribbon is still there**: View › Classic Ribbon (⌃⌘R).

## Install

Download `optimo-<version>-macos-universal.dmg` from [Releases](../../releases), open it and drag
Optimo to Applications. macOS 11 or later, Apple silicon and Intel.

## Build from source

```bash
cargo run --release -p wordcraft -- --sample
```

Package a signed app: see [`docs/releasing.md`](docs/releasing.md). `packaging/macos/package.sh`
defaults to Optimo's branding (`APP_NAME=Optimo BUNDLE_ID=ai.spert.optimo
APP_ICNS=assets/app-icon/optimo.icns FILE_PREFIX=optimo`). Run it from source with
`scripts/optimo-run.sh` (sample document + control channel on port 7981).

Settings live in `~/Library/Application Support/Optimo`; on first start Optimo copies them from
Margin's folder (or WordCraft's) and copies the assistant API keys from Margin's keychain entries.

## How it relates to WordCraft

Optimo is a fork that changes the interface only; the engine is WordCraft's. All Optimo code lives
in a few files (`crates/ui-egui/src/skin.rs`, `mac_ui.rs`, `mac_menu.rs`,
`apps/wordcraft/src/mac_menu.rs`) with small hooks elsewhere, so upstream changes merge easily.
We offer the interface back to WordCraft as pull requests. Design decisions:
[`docs/decisions.md`](docs/decisions.md). WordCraft's own README: [`README-WordCraft.md`](README-WordCraft.md).

## License

MIT OR Apache-2.0, like WordCraft. Every bundled asset is listed in [`ATTRIBUTION.md`](ATTRIBUTION.md).
