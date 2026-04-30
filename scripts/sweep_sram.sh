#!/usr/bin/env bash
set -euo pipefail

EDGE_LIST="${1:-data/twitter-2010/twitter-2010-t.txt}"
LIMIT_EDGES="${LIMIT_EDGES:-100000000}"
PROGRESS_EVERY="${PROGRESS_EVERY:-10000000}"
JSON_DIR="${JSON_DIR:-}"

if [ -n "$JSON_DIR" ]; then
  mkdir -p "$JSON_DIR"
fi

for sets in 1024 2048 4096 8192 16384 32768 65536; do
  echo "sets=${sets}"
  json_args=()
  if [ -n "$JSON_DIR" ]; then
    json_args=(--json "$JSON_DIR/edge_stream_sets${sets}.json")
  fi

  cargo run --release -- run \
    --edge-list "$EDGE_LIST" \
    --limit-edges "$LIMIT_EDGES" \
    --grid 8 \
    --lanes 64 \
    --stages 4 \
    --sets "$sets" \
    --ways 4 \
    --progress-every "$PROGRESS_EVERY" \
    "${json_args[@]}"
done
