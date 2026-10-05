# 0002: Sorted sets and scan

Status: accepted

## Context

Sorted sets (zsets) and key pattern matching were added to squall. Both have
large Redis surfaces; we had to decide what to copy, where to deviate, and
which std-only data structure backs a zset.

## Decision

Follow ADR 0001 (Redis-faithful names, borrowed reads, `&mut self`, one
`Error` enum) and make these choices:

- **Zset API.** `zadd`, `zscore`, `zrem`, `zcard`, `zrange`,
  `zrange_withscores`, `zrangebyscore`, `zrangebyscore_withscores`.
  Score ranges take a public `Bound` enum (`Inclusive(f64)`,
  `Exclusive(f64)`). Reads return borrowed `Vec<&[u8]>` or
  `Vec<(&[u8], f64)>`.
- **No `ZADD` flags, no `zincrby`.** No NX/XX/GT/LT/CH/INCR, no REV, BYSCORE
  or LIMIT options. `zadd` returns the count of newly added members only;
  updating an existing member's score is not counted.
- **NaN.** A NaN score or bound fails with the new `Error::NotAFloat`,
  checked before the key's type, with no change made. Infinite scores are
  allowed.
- **Ordering.** By score, then member bytes. `-0.0 == 0.0`; `zscore` returns
  the stored value.
- **`scan(pattern)` is a single-call `KEYS pattern`**, not a cursor
  iterator. It returns `Vec<Vec<u8>>` for keys of every type, using full
  Redis glob (`*`, `?`, classes, ranges, negation, `\` escape), byte-based,
  never erroring on odd patterns. `keys()` is unchanged (`KEYS *`).
- **Data structure.** A private `BTreeSet<(Score, Vec<u8>)>` for order plus
  a `HashMap<Vec<u8>, Score>` for member lookup, std only. `Score` is a private
  total-order wrapper over non-NaN `f64`.

## Alternatives considered

- **Skip list with spans** (as Redis does) for O(log n) rank access.
  Rejected: needs unsafe or heavy indexing code for a small library.
- **Sorted `Vec`**. Rejected: O(n) insert and remove.
- **Cursor-based `scan`** like Redis `SCAN`. Rejected: a cursor protects
  against concurrent writers, and `&mut self` access has none.
- **Supporting `ZADD` flags and `zincrby` now.** Deferred: not needed for the
  core model; can be added later without breaking callers.

## Consequences

- `zrangebyscore` seeks in O(log n), but index-range `zrange` must skip to
  `start` in O(n). Rank access is a known limitation.
- Insert, update, remove and score lookup are O(log n) or better, at the
  cost of storing each member twice.
- `Error` gained a variant; it is `#[non_exhaustive]`, so this does not break
  callers.
- Callers needing Redis `SCAN` incremental iteration or `ZADD` flags must
  wait for a later change.
