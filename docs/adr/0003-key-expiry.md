# 0003: Key expiry

Status: accepted

## Context

Callers need Keys that disappear on their own after a set time (caches,
sessions, rate-limit windows), as Redis `EXPIRE` gives them. squall has no
background threads and no internal locking (ADR 0001), reads take `&self`,
and tests must be deterministic. We had to decide how Expired keys are
removed, where time comes from, and how writes treat an existing TTL.

## Decision

- **Commands.** `expire(key, Duration) -> bool`, `ttl(key) -> Ttl`
  (`Missing`, `Persistent`, `Remaining(Duration)`, exact), `persist(key) ->
  bool` and `purge_expired() -> usize`. No
  `EXPIREAT`, flags, `GETEX` or `EXPIRETIME`. `Duration::ZERO` expires the
  Key at once; a deadline that would overflow `Instant` means no TTL. Expiry
  commands are infallible.
- **Drain via a deadline index.** Beside the value map the Store keeps a
  deadline map (Key to `Instant`) and a `BTreeSet<(Instant, Key)>` ordered by
  deadline, kept in sync by the private Keyspace helpers. A Key is expired
  when `deadline <= now`, checked in one place. Every `&mut self` command
  first removes every due Key from the index; when nothing is due this is
  one look at the earliest deadline. `&self` reads never delete; they treat a
  due Key as missing. `purge_expired()` runs the same drain on demand and
  returns the count.
- **Injectable monotonic clock.** Time is a monotonic `Instant` from a
  `Send + Sync` `Clock` trait object. `Store::new()` uses `SystemClock`;
  `Store::with_clock` takes any `Clock`. `ManualClock` is a public handle
  whose clones share one time and which only moves forward with `advance`,
  so squall's tests and callers' tests are deterministic without sleeping.
  A command reads the clock at most once: `expire` always reads it, any
  other mutating command not at all when no Key in the Store has a TTL, and
  a read not at all when the Key it reads has none.
- **Redis TTL-on-write semantics.** `set` clears the TTL, `expire`
  replaces it, `persist` removes it. In-place writes (`append`,
  counters, pushes and pops, `hset`/`hdel`, `sadd`/`srem`, `zadd`/`zrem`)
  keep it, so the INCR + EXPIRE rate-limit pattern works. `del` and the
  removal of an emptied container drop it; a recreated Key has no TTL.
- **Errors.** A failed command still makes no observable change. The drain
  at its start may run before the error, but it only removes Keys that were
  already invisible; the only trace is a lower count from a later
  `purge_expired`. This refines ADR 0001's "a failed command changes
  nothing".

## Alternatives considered

- **Touch-only reaping plus a caller-driven purge** (Expired keys removed
  only when a command touches them). Rejected: memory grows without bound
  unless callers remember to purge, and an exact `len` costs O(expired).
- **Background expiry thread.** Rejected: needs internal locking, which ADR
  0001 rules out, and makes tests timing-dependent.
- **Random sampling** (Redis active expiry). Rejected: memory is only
  probabilistically bounded and results are not deterministic.
- **Generic `Store<C: Clock>`.** Rejected: the type parameter leaks into
  every caller's signatures for a choice most never make.
- **Interior mutability so reads can delete.** Rejected: `&self` reads would
  need `RefCell` (losing `Sync`) or an internal lock (ruled out by ADR 0001),
  and a read could not hand out borrows while it might delete.
- **Wall-clock `SystemTime` deadlines.** Rejected: changing the system clock
  would expire or revive Keys.

## Consequences

- A Store that never uses TTLs behaves and performs as before: drains and
  reads skip the clock when no Key has a TTL.
- Each Key with a TTL is stored once more in the index; setting or clearing
  a TTL costs O(log n). The first write after many Keys share a deadline
  pays to remove all of them; a cap per drain can be added if measurements
  show latency spikes.
- Memory used by Expired keys is reclaimed on the next write, or by
  `purge_expired()` during read-only phases.
- `Store`'s `Debug` and `Default` are hand-written; it stays `Send + Sync`
  with any `Clock`.
