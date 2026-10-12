# squall

A small in-memory key-value store in Rust, inspired by the data types of
[Redis](https://redis.io). A *squall* is a sudden, short storm: small and
fast.

## What it is

A library of data structures you use from Rust code:

- a **keyspace**: set, get, delete, check and list keys, plus `scan(pattern)`
  to find keys by Redis-style glob (a single call, like `KEYS pattern`, no
  cursor);
- **strings**, **lists**, **hashes**, **sets** and **sorted sets** as values.

Sorted sets (`zadd`, `zscore`, `zrem`, `zcard`, `zrange`, `zrange_withscores`,
`zrangebyscore`, `zrangebyscore_withscores`) order members by `f64` score,
then member bytes. Score ranges take a `Bound` (`Inclusive` or `Exclusive`).
A NaN score or bound fails with `Error::NotAFloat`. Not supported: `ZADD`
flags and `zincrby`. Index-range `zrange` is O(n) to skip to the start.

Keys can expire, like Redis `EXPIRE`: `set_ex(key, value, Duration)` writes a
string with a TTL, `expire` gives an existing Key one, `ttl` reports the time
left (`Ttl::Missing`, `Persistent` or `Remaining`), `persist` removes it and
`purge_expired` removes every Expired key now. `set` clears a TTL; in-place
writes such as `incr`, `append` or `lpush` keep it. Time comes from a `Clock`;
`Store::with_clock(ManualClock::new())` makes tests deterministic. Expired
keys are reclaimed on the next write or by `purge_expired()`.

## What it isn't

No server or network protocol and no persistence to disk. It's the
data-structure layer only.

## Build and test

```
cargo build
cargo test
```

## How it's built

squall is being built with [Marut](https://github.com/himanshukandwal/marut),
a Claude Code plugin for spec-first, multi-agent work, as a real-world test
of that plugin.
