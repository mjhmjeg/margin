#!/bin/sh
# Build the Margin desktop app and launch it with the sample document and a control channel.
# Refuses to launch a binary older than the newest source file (it happened twice on 2026-10-12).
set -e
cd "$(dirname "$0")/.."
. "$HOME/.cargo/env"
cargo build --release -q -p wordcraft --bin wordcraft
bin=target/release/wordcraft
newest=$(find crates apps -name '*.rs' -newer "$bin" | head -1)
[ -z "$newest" ] || { echo "stale binary: $newest is newer than $bin" >&2; exit 1; }
pkill -f "target/release/wordcraft" 2>/dev/null || true
sleep 1
nohup "$bin" --sample --control 7981 >/dev/null 2>&1 &
echo "launched $bin ($(date -r "$bin" '+%H:%M:%S'))"
