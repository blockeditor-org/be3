# Programs use the system allocator

be-host and beui programs allocate through the system allocator; no replacement global
allocator is linked.

## Why

The cost a faster allocator hides is the number of allocations. Allocating less removes it;
a different allocator only works around it.

## Rejected

- jemalloc: about 15% faster. Kept in reserve for when nothing better is left.
- mimalloc: about 6% faster. Kept in reserve for the same reason.
