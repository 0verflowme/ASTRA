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
  --progress-every 10000000 \
  --json results/twitter2010_100m_sets4096.json
```

## Direct BVGraph Streaming

Sequential BVGraph streaming avoids materializing a huge text edge list. The basename is the path without the `.graph` or `.properties` extension:

```bash
cargo run --release -- run-bvgraph \
  --basename data/twitter-2010/twitter-2010-t \
  --grid 8 \
  --lanes 64 \
  --epoch 1 \
  --target dst \
  --stages 4 \
  --sets 4096 \
  --ways 4 \
  --limit-edges 100000000 \
  --progress-every 10000000 \
  --json results/twitter2010_bvgraph_100m_sets4096.json
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
  --ways 4 \
  --json results/smoke_trace.json
```

BVGraph traces are generated directly from compressed graph files:

```bash
cargo run --release -- gen-trace-bvgraph \
  --basename data/twitter-2010/twitter-2010-t \
  --out traces/twitter2010_t_bfs_orand.bin \
  --limit-edges 100000000 \
  --progress-every 10000000
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
external_hits
internal_merge_hits
admitted
bypassed
eviction_swaps
eviction_flushes
drained
hit_rate
external_hit_rate
total_merge_rate
bypass_rate
compression
owner_queue_max
owner_queue_mean
```

`hit_rate` is the external incoming-packet hit rate. `table_hits` is the total
of external hits plus internal merge hits caused by evicted aggregates merging
downstream.

`run`, `run-bvgraph`, and `simulate` accept `--json PATH` to write the same
report as structured JSON for plotting and sweep automation.

## Phase 1 Scope

Implemented:

- Text edge-list streaming.
- Sequential BVGraph streaming from `.graph` and `.properties`.
- BFS-style packet generation.
- OR reduction over 64-bit lane masks.
- Deterministic owner mapping and switch set hashing.
- HashPipe-style staged switch reduce cache.
- Lossless bypass, eviction flush, and drain accounting.
- Binary packet traces with a v1 header.

Deferred:

- Real BFS frontier and visited filtering.
- Min-plus and other semiring reductions.
- NIC coalescing and watermarks.
- Multi-switch topology.

## License

ASTRA-Sim is licensed under either MIT or Apache-2.0, at your option. See
`LICENSE-MIT` and `LICENSE-APACHE`.
