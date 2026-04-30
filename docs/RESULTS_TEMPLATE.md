# Results Template

```text
dataset=twitter-2010-t
mode=bfs_orand_edge_stream
target=dst
lanes=64
grid=8x8
edges_used=100000000
```

```text
stages  sets   ways  entries  packets_in  packets_out  external_hit_rate  total_merge_rate  bypass_rate  compression  owner_queue_max
4       1024   4     16384    ...         ...          ...                ...               ...          ...          ...
4       2048   4     32768    ...         ...          ...                ...               ...          ...          ...
4       4096   4     65536    ...         ...          ...                ...               ...          ...          ...
4       8192   4     131072   ...         ...          ...                ...               ...          ...          ...
4       16384  4     262144   ...         ...          ...                ...               ...          ...          ...
4       32768  4     524288   ...         ...          ...                ...               ...          ...          ...
4       65536  4     1048576  ...         ...          ...                ...               ...          ...          ...
```

Accounting check:

```text
packets_out == bypassed + eviction_flushes + drained
table_hits == external_hits + internal_merge_hits
```
