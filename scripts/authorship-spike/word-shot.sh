#!/bin/bash
# Screenshot of Microsoft Word's windows only, by window id (never the whole desktop: the
# machine runs other work). Used by run.sh when a Word round trip hangs, to see whether Word
# shows a repair / "unreadable content" dialog. Output stays under plan/word/ (gitignored).
#
# usage: word-shot.sh out.png   -> out.png for the first window, out-2.png for the next…
set -euo pipefail
out=${1:?usage: word-shot.sh out.png}
here=$(cd "$(dirname "$0")" && pwd)
ids=$(swift "$here/window-ids.swift" "Microsoft Word" | cut -f1)
if [ -z "$ids" ]; then echo "no Word window on screen" >&2; exit 1; fi
n=0
for id in $ids; do
  n=$((n + 1)); f=$out; [ "$n" -gt 1 ] && f="${out%.png}-$n.png"
  screencapture -x -l"$id" "$f" && echo "$f"
done
