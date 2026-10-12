#!/usr/bin/env bash
# Margin fork: render assets/app-icon/margin.svg to the macOS icon files (outputs are committed).
# Needs resvg (cargo install resvg) and iconutil. Same Apple grid as packaging/icons.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIR="$ROOT/assets/app-icon"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
MAC="$TMP/mac.svg"
sed 's/viewBox="0 0 512 512"/viewBox="-62.136 -62.136 636.272 636.272"/' "$DIR/margin.svg" >"$MAC"
render() { resvg -w "$2" -h "$2" "$1" "$3" </dev/null; }
render "$DIR/margin.svg" 1024 "$DIR/margin-1024.png"
render "$MAC" 512 "$DIR/margin-macos-512.png"
SET="$TMP/margin.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  render "$MAC" "$s" "$SET/icon_${s}x${s}.png"
  render "$MAC" "$((s * 2))" "$SET/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$SET" -o "$DIR/margin.icns"
ls -la "$DIR"/margin*
