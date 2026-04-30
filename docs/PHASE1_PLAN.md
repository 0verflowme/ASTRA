# Phase 1 Plan

ASTRA-Sim Phase 1 builds a deterministic Rust trace/direct-stream simulator for BFS-style OR/AND sparse packet traffic over an 8x8 chip grid, measuring whether a finite-SRAM lossless HashPipe-style switch reduce cache can compress power-law graph traffic enough to justify ASTRA's algebraic network fabric.

## Workload

The first workload is BFS-style frontier expansion:

```text
op    = OR
block = vertex / lanes
mask  = 1 << (vertex % lanes)
value = unused
```

The input can be a whitespace-separated edge list:

```text
src dst
src dst
...
```

or a sequential BVGraph basename with companion files:

```text
BASENAME.graph
BASENAME.properties
```

The default target is `dst`.

## Switch Model

The default cache is:

```text
stages = 4
sets   = 4096
ways   = 4
```

For each packet:

```text
hit:
    reduce OR mask into resident entry

empty:
    admit packet

full:
    keep resident entry unless carried entry has proven more reuse
    push evicted aggregate to next stage only on strict hit-count win
    otherwise continue carrying the incoming packet or aggregate

leaves final stage:
    count as packets_out
    charge owner_queue[dst_chip]

end:
    drain all valid entries
```

Overflow only reduces compression. It does not change correctness because emitted aggregates are still delivered to the owner chip.

The default replacement policy is cold-bypass: a brand-new cold key does not evict
an equally cold resident solely because it is newer. This keeps the reduce cache
biased toward keys with observed reuse rather than recency churn.

## Determinism

Switch set indexing uses a custom deterministic hash over `PacketKey` and stage. It does not use Rust's randomized `DefaultHasher`.

## Trace Format

Trace files use a v1 header followed by fixed 32-byte packet records:

```text
magic       = b"ASTRATRC"
version     = 1
record_size = 32
lanes       = 1..=64
grid        > 0
packet_count = u64::MAX for unknown/streaming
```

Packet record:

```text
dst_chip:u16
epoch:u16
op:u8
flags:u8
reserved:u16
block:u64
mask:u64
value:u64
```

## First Experiments

```bash
cargo run --release -- smoke --out data/smoke.edgelist
cargo run --release -- run --edge-list data/smoke.edgelist
```

```bash
cargo run --release -- run \
  --edge-list data/twitter-2010/twitter-2010-t.txt \
  --limit-edges 100000000 \
  --sets 4096 \
  --ways 4 \
  --stages 4 \
  --progress-every 10000000
```

Preferred compressed-graph path:

```bash
cargo run --release -- run-bvgraph \
  --basename data/twitter-2010/twitter-2010-t \
  --limit-edges 100000000 \
  --sets 4096 \
  --ways 4 \
  --stages 4 \
  --progress-every 10000000
```
