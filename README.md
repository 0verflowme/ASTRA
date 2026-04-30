# ASTRA-Sim Phase 1

ASTRA-Sim Phase 1 is a deterministic Rust simulator for BFS-style OR/AND sparse packet traffic over an 8x8 chip grid. It measures whether a finite-SRAM, lossless HashPipe-style switch reduce cache can compress power-law graph traffic enough to justify ASTRA's algebraic network fabric.

## Build

```bash
cargo build --release
```

## Smoke Test

```bash
cargo run --release -- smoke --out data/smoke.edgelist
cargo run --release -- run --edge-list data/smoke.edgelist
```

## Direct Edge Streaming

```bash
cargo run --release -- run \
  --edge-list data/twitter-2010/twitter-2010-t.txt \
  --grid 8 \
  --lanes 64 \
  --epoch 1 \
  --target dst \
  --stages 4 \
  --sets 4096 \
  --ways 4 \
  --progress-every 10000000
```

## Generate and Simulate a Trace

```bash
cargo run --release -- gen-trace \
  --edge-list data/smoke.edgelist \
  --out traces/smoke.bin

cargo run --release -- simulate \
  --trace traces/smoke.bin \
  --stages 4 \
  --sets 4096 \
  --ways 4
```

## Metrics

The simulator prints stable key-value lines. The core accounting rule is:

```text
packets_out = bypassed packets + eviction-flushed packets + drained packets
compression = packets_in / max(packets_out, 1)
```

Important outputs:

```text
packets_in
packets_out
table_hits
admitted
bypassed
eviction_swaps
eviction_flushes
drained
hit_rate
bypass_rate
compression
owner_queue_max
owner_queue_mean
```

## Phase 1 Scope

Implemented:

- Text edge-list streaming.
- BFS-style packet generation.
- OR reduction over 64-bit lane masks.
- Deterministic owner mapping and switch set hashing.
- HashPipe-style staged switch reduce cache.
- Lossless bypass, eviction flush, and drain accounting.
- Binary packet traces with a v1 header.

Deferred:

- Real BFS frontier and visited filtering.
- Direct compressed WebGraph reading.
- Min-plus and other semiring reductions.
- NIC coalescing and watermarks.
- Multi-switch topology.
