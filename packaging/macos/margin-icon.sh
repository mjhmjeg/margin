#!/usr/bin/env bash
# Margin fork: render assets/app-icon/margin.svg (+ margin-small.svg) to the macOS icon files (outputs are committed).
# Needs resvg (cargo install resvg) and iconutil. Same Apple grid as packaging/icons.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIR="$ROOT/assets/app-icon"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
# The design handoff artwork (1024 grid) already carries the macOS tile padding, so it is
# rendered as is. margin-small.svg (text lines dropped, thicker rule) is used at 32 pt and below.
render() { resvg -w "$2" -h "$2" "$1" "$3" </dev/null; }
render "$DIR/margin.svg" 1024 "$DIR/margin-1024.png"
render "$DIR/margin.svg" 512 "$DIR/margin-macos-512.png"
SET="$TMP/margin.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  for px in "$s" "$((s * 2))"; do
    src="$DIR/margin.svg"
    if [ "$px" -le 64 ]; then src="$DIR/margin-small.svg"; fi
    if [ "$px" = "$s" ]; then name="icon_${s}x${s}.png"; else name="icon_${s}x${s}@2x.png"; fi
    render "$src" "$px" "$SET/$name"
  done
done
iconutil -c icns "$SET" -o "$DIR/margin.icns"
ls -la "$DIR"/margin*
