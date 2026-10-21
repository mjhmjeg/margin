#!/usr/bin/env bash
# Optimo fork: render assets/app-icon/optimo.svg (+ optimo-small.svg) to the macOS icon files (outputs are committed).
# Needs resvg (cargo install resvg) and iconutil. Same Apple grid as packaging/icons.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIR="$ROOT/assets/app-icon"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
# The artwork (1024 grid) already carries the macOS tile padding, so it is rendered as is.
# optimo-small.svg (thicker stroke, three bigger dots) is used at 32 pt and below.
render() { resvg -w "$2" -h "$2" "$1" "$3" </dev/null; }
render "$DIR/optimo.svg" 1024 "$DIR/optimo-1024.png"
render "$DIR/optimo.svg" 512 "$DIR/optimo-macos-512.png"
SET="$TMP/optimo.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  for px in "$s" "$((s * 2))"; do
    src="$DIR/optimo.svg"
    if [ "$px" -le 64 ]; then src="$DIR/optimo-small.svg"; fi
    if [ "$px" = "$s" ]; then name="icon_${s}x${s}.png"; else name="icon_${s}x${s}@2x.png"; fi
    render "$src" "$px" "$SET/$name"
  done
done
iconutil -c icns "$SET" -o "$DIR/optimo.icns"
ls -la "$DIR"/optimo*
