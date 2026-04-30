# ASTRA Brief

ASTRA is a rack-scale sparse algebra machine that turns irregular graph computation into streamed semiring packets, reducing them across a lossless algebraic network fabric.

The core primitive is sparse semiring multiplication:

```text
C[i,j] = reduce over k of A[i,k] multiply B[k,j]
```

Different semirings map to different workloads:

```text
(+, *)      dense and sparse matrix multiplication
(min, +)    shortest paths and routing
(or, and)   BFS and reachability
(max, +)    Viterbi-style dynamic programming
```

ASTRA's Phase 1 simulator tests one key physical question:

```text
Can finite switch SRAM compress BFS-like sparse graph traffic through lossless in-network reduction?
```

If compression is weak, the architecture needs stronger NIC aggregation, partitioning, hub pinning, or larger packet lanes. If compression is strong, the algebraic fabric has a plausible path toward RTL and hardware experiments.
