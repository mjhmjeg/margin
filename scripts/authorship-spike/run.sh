#!/bin/bash
# A5 spike: how authorship marks survive Word and WordCraft (docs/authorship-storage.md).
#
# Builds the harness (crates/docx/examples/authorship_spike.rs), writes one synthetic document
# per candidate, then round-trips each through
#   - this repo's wordcraft-cli (Margin): a plain open+save, and a save after scripted edits
#     inside / at the edge of the AI spans;
#   - upstream WordCraft's wordcraft-cli, when UPSTREAM_CLI points at one
#     (git worktree add /tmp/wc upstream/main && CARGO_TARGET_DIR=/tmp/wc-target cargo build -p wordcraft-cli);
#   - Microsoft Word through its AppleScript object model, when Word is installed (never
#     keystrokes or clicks; see word-roundtrip.applescript),
# and inspects every result with `authorship_spike check`. A summary table comes last.
#
# Everything lands in plan/word/authorship-spike/ (gitignored): files produced by Word are
# never committed. Re-running overwrites; the generated sources are byte-identical every time.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${OUT:-$ROOT/plan/word/authorship-spike}
UPSTREAM_CLI=${UPSTREAM_CLI:-}
WORD_TIMEOUT=${WORD_TIMEOUT:-180}
# f-feature is not a synthetic candidate: it is the A5 feature's own output, a document whose
# assistant suggestions were accepted in Margin (hidden `_MarginAI_` bookmarks + the
# `urn:margin:authorship` store), with the same text and spans as the candidates.
CANDIDATES=(a-bookmarks b-offsets c-ignorable d-sdt e-style f-feature)
STAGES=(src margin margin-edit upstream upstream-edit word word-edit)

# The scripted edits, as WordCraft commands: a paragraph-start insertion (shifts every offset),
# an insertion inside span 1, a deletion at the start of span 2, typing right after span 3.
EDITS=(--cmd caret.docStart --cmd 'text.insert={"text":"Edited. "}'
  --cmd 'select.text={"text":"river"}' --cmd 'text.insert={"text":"wide river"}'
  --cmd 'select.text={"text":"Seewis "}' --cmd text.delete
  --cmd 'select.text={"text":"good boots"}' --cmd caret.right --cmd 'text.insert={"text":" AFTER"}')

# shellcheck disable=SC1091
. "$HOME/.cargo/env" 2>/dev/null || true
cd "$ROOT"
cargo build -q -p wordcraft-docx --example authorship_spike
cargo build -q -p wordcraft-cli
SPIKE=$ROOT/target/debug/examples/authorship_spike
MARGIN_CLI=$ROOT/target/debug/wordcraft-cli

check() { "$SPIKE" check "$1" > "${1%.docx}.check.txt"; }
summary() { sed -n 's/^summary //p' "${1%.docx}.check.txt"; }

# with_timeout SECONDS cmd…: like coreutils timeout (not on macOS).
with_timeout() {
  local secs=$1 rc=0; shift
  "$@" & local pid=$!
  ( sleep "$secs"; kill "$pid" 2>/dev/null ) & local killer=$!
  wait "$pid" || rc=$?
  kill "$killer" 2>/dev/null; wait "$killer" 2>/dev/null || true
  return $rc
}

roundtrip_cli() { # name cli
  local name=$1 cli=$2 f
  mkdir -p "$OUT/$name" "$OUT/$name-edit"
  for f in "${CANDIDATES[@]}"; do
    "$cli" convert "$OUT/src/$f.docx" "$OUT/$name/$f.docx" 2>/dev/null
    check "$OUT/$name/$f.docx"
    "$cli" run --file "$OUT/src/$f.docx" "${EDITS[@]}" --save "$OUT/$name-edit/$f.docx" 2>/dev/null
    check "$OUT/$name-edit/$f.docx"
  done
}

word_present() { osascript -e 'tell application id "com.microsoft.Word" to get version' >/dev/null 2>&1; }

roundtrip_word() {
  local f mode dir anchor
  mkdir -p "$OUT/word" "$OUT/word-edit" "$OUT/screenshots"
  for f in "${CANDIDATES[@]}"; do
    case $f in a-bookmarks) anchor=bookmark ;; d-sdt) anchor=sdt ;; *) anchor=none ;; esac
    for mode in plain edit; do
      dir=word; [ "$mode" = edit ] && dir=word-edit
      rm -f "$OUT/$dir/$f.docx"
      if with_timeout "$WORD_TIMEOUT" osascript "$ROOT/scripts/authorship-spike/word-roundtrip.applescript" \
          "$OUT/src/$f.docx" "$OUT/$dir/$f.docx" "$mode" "$anchor" > "$OUT/$dir/$f.applescript.log" 2>&1 \
         && [ -f "$OUT/$dir/$f.docx" ]; then
        check "$OUT/$dir/$f.docx"
      else
        echo "word $mode $f: osascript failed or timed out (a repair dialog?) - see $OUT/$dir/$f.applescript.log" >&2
        "$ROOT/scripts/authorship-spike/word-shot.sh" "$OUT/screenshots/$f-$mode.png" || true
        with_timeout 30 osascript -e 'tell application id "com.microsoft.Word" to quit saving no' >/dev/null 2>&1 || true
      fi
    done
  done
}

# The feature's own document: the assistant (author "Claude", tracked) writes the three spans,
# a person writes the rest, then the person accepts the assistant's changes (review.acceptAll by
# author), which wraps each accepted insertion in a mark and records it in the store.
make_feature() {
  local out=$1
  "$MARGIN_CLI" run --template blank --author Person \
    --cmd 'text.insert={"text":"The valley wakes slowly. ","raw":true}' \
    --cmd 'file.setAuthor={"name":"Claude"}' --cmd 'review.trackChanges={"value":true}' \
    --cmd 'text.insert={"text":"Mist lifts off the river by eight,","raw":true}' \
    --cmd 'review.trackChanges={"value":false}' --cmd 'file.setAuthor={"name":"Person"}' \
    --cmd 'text.insert={"text":" and the bakery opens at nine.","raw":true}' \
    --cmd text.newParagraph \
    --cmd 'file.setAuthor={"name":"Claude"}' --cmd 'review.trackChanges={"value":true}' \
    --cmd 'text.insert={"text":"Seewis sits above the Prättigau, a village of six hundred people and a view that pays for the climb.","raw":true}' \
    --cmd 'review.trackChanges={"value":false}' --cmd 'file.setAuthor={"name":"Person"}' \
    --cmd text.newParagraph \
    --cmd 'text.insert={"text":"Notes for the trip: ","raw":true}' \
    --cmd 'file.setAuthor={"name":"Claude"}' --cmd 'review.trackChanges={"value":true}' \
    --cmd 'text.insert={"text":"pack a map and ","raw":true}' --cmd format.bold \
    --cmd 'text.insert={"text":"good boots","raw":true}' --cmd format.bold \
    --cmd 'review.trackChanges={"value":false}' --cmd 'file.setAuthor={"name":"Person"}' \
    --cmd 'text.insert={"text":" before Friday.","raw":true}' \
    --cmd text.newParagraph \
    --cmd 'text.insert={"text":"A closing line written by a person.","raw":true}' \
    --cmd 'review.acceptAll={"author":"Claude"}' \
    --save "$out" > /dev/null
}

mkdir -p "$OUT/src"
"$SPIKE" make "$OUT/src" > /dev/null
make_feature "$OUT/src/f-feature.docx"
for f in "${CANDIDATES[@]}"; do check "$OUT/src/$f.docx"; done

roundtrip_cli margin "$MARGIN_CLI"
if [ -n "$UPSTREAM_CLI" ]; then roundtrip_cli upstream "$UPSTREAM_CLI"; else echo "UPSTREAM_CLI not set: upstream round trip skipped" >&2; fi
if word_present; then roundtrip_word; else echo "Microsoft Word not installed (or automation refused): Word round trip skipped" >&2; fi

echo
printf '%-13s %-14s %s\n' candidate stage summary
for f in "${CANDIDATES[@]}"; do
  for s in "${STAGES[@]}"; do
    [ -f "$OUT/$s/$f.check.txt" ] && printf '%-13s %-14s %s\n' "$f" "$s" "$(summary "$OUT/$s/$f.docx")"
  done
done
echo
echo "reports: $OUT/<stage>/<candidate>.check.txt (diff src vs a stage to see what changed)"
