#!/usr/bin/env bash
set -euo pipefail

BASENAME="${1:-data/twitter-2010/twitter-2010-t}"
LIMIT_EDGES="${LIMIT_EDGES:-100000000}"
PROGRESS_EVERY="${PROGRESS_EVERY:-10000000}"

for sets in 1024 2048 4096 8192 16384 32768 65536; do
  echo "sets=${sets}"
  cargo run --release -- run-bvgraph \
    --basename "$BASENAME" \
    --limit-edges "$LIMIT_EDGES" \
    --grid 8 \
    --lanes 64 \
    --stages 4 \
    --sets "$sets" \
    --ways 4 \
    --progress-every "$PROGRESS_EVERY"
done
