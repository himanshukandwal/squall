# squall: domain context

squall is an in-memory key-value store library with Redis-style data types.
This file defines the vocabulary used in the code, docs and tests.

## Terms

**Store**
The single public type. A `Store` is the Keyspace plus every command that
operates on it. Commands are methods named after their Redis equivalent
(`set`, `lpush`, `hset`, `sadd`, ...). A `Store` is a plain `&mut self` type
with no internal locking.

**Keyspace**
The mapping held by a Store from Keys to Values. Each Key appears at most
once and holds exactly one Value. An emptied list, hash, set or zset is
removed from the Keyspace, so a Keyspace never contains an empty container.

**Key**
The name a Value is stored under. Arbitrary bytes (binary-safe). Callers may
pass `&str`, `String`, `&[u8]` or `Vec<u8>`.

**Value**
What a Key holds: one datum of exactly one Type. The internal value enum is
never exposed. Strings, list elements, hash fields and values, set and zset
members are all arbitrary bytes. Zset scores are `f64`.

**Type**
Which of the five kinds a Value is. A command run on a Key holding a
different Type fails with `WrongType`. The exceptions are `set` and
`set_ex`, which overwrite a Key of any Type.

### The five types

**string**
A byte string. Supports `append`, `strlen` and the counters (`incr`, `decr`,
`incrby`). Counters read and write the string as decimal text of an `i64`.

**list**
An ordered sequence of byte strings with push and pop at both ends. Indices
are `i64`; negative indices count from the end (`-1` is the last element).

**hash**
A map from field (bytes) to value (bytes), stored under one Key. Field order
is unspecified.

**set**
An unordered collection of unique byte-string members. Member order is
unspecified.

**zset**
A sorted set: unique byte-string members, each with an `f64` score. Members
are ordered by score, ties broken by member bytes. `-0.0` and `0.0` compare
equal; `zscore` returns the stored value. Infinite scores are allowed. Index
ranges (`zrange`) use list-style indices (negative from the end, inclusive
stop, clamped). Commands: `zadd`, `zscore`, `zrem`, `zcard`, `zrange`,
`zrange_withscores`, `zrangebyscore`, `zrangebyscore_withscores`. Reads
return borrowed `Vec<&[u8]>` or `Vec<(&[u8], f64)>`.

**Bound**
One end of a score range for `zrangebyscore`: `Bound::Inclusive(f64)` or
`Bound::Exclusive(f64)`. Use `f64::NEG_INFINITY` / `f64::INFINITY` for an open
end. Min above max gives an empty result.

**scan**
`scan(pattern) -> Vec<Vec<u8>>` returns every Key (of any Type) matching a
Redis-style glob (`*`, `?`, `[abc]`, `[a-z]`, `[^a]`, `\` escape), byte-based.
A single call, like Redis `KEYS pattern`; there is no cursor. A pattern never
errors. `keys()` returns all keys, like `KEYS *`.

## Behaviour terms

**Missing key**
A Key not in the Keyspace, or an Expired key. Reads treat it as empty (`None`,
an empty result, or 0), never `WrongType`, and `len`, `keys` and `scan` leave
it out. Creating writes (`append`, pushes, `hset`, `sadd`, `zadd`, counters) create it.

**TTL / deadline**
A Key may carry a TTL (time to live), set with `set_ex` or `expire` and
removed with `persist`; `set` clears it and in-place writes keep it. It is
stored as a deadline: the `Instant` at which the Key expires. A Key without
one never expires. `expire` on a missing Key does nothing and returns
`false`; `Duration::ZERO` deletes the Key at once; a deadline that would
overflow `Instant` means no TTL. `del` and the removal of an emptied
container drop the TTL, so a recreated Key starts without one.

**Expired key**
A Key whose deadline is `<= now`. Every command treats it as a Missing key.
Reads (`&self`) never delete it; every mutating command first removes all
Expired keys from memory.

**Clock**
The `Send + Sync` trait a Store reads the current `Instant` from, chosen at
creation: `Store::new()` uses `SystemClock` (the system's monotonic clock);
`Store::with_clock` takes any `Clock`. `ManualClock` is a handle for tests
whose clones share one time; it starts at the real now and moves only
forward with `advance(Duration)`.

**Ttl**
What `ttl(key)` returns: `Ttl::Missing` (missing or Expired key, Redis
`-2`), `Ttl::Persistent` (no TTL, Redis `-1`) or `Ttl::Remaining(Duration)`
(the exact time left).

**purge_expired**
`purge_expired() -> usize` removes every Expired key from memory now and
returns how many. Only needed during read-only phases, since every mutating
command already does this first.

**WrongType / NotAnInteger / Overflow / NotAFloat**
The variants of the single `Error` enum (`#[non_exhaustive]`). A command that
returns an error has made no change to the Store.
`NotAFloat` means a zset score or score bound is NaN; it is checked before
the key's Type, as Redis does.

## Not in scope

No server or network protocol or persistence. See
`docs/adr/` for design decisions.
