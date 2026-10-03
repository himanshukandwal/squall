# 0001: Redis semantics, Rust ownership

Status: accepted

## Context

squall gives Rust programs the Redis data model (strings, lists, hashes, sets
under binary-safe keys) as an in-process library. We had to decide how
closely to copy Redis, and where to follow Rust idioms instead.

## Decision

Mirror Redis behaviour, but express it with Rust's ownership rules:

- The API is Redis-faithful: one `Store` with methods named after Redis
  commands. Wrong-type operations fail, missing keys read as empty, emptied
  containers disappear, counters are decimal text, list indices may be
  negative.
- Reads borrow. `get`, `hget`, `lindex`, `lrange` and the like return
  slices into the Store; the borrow ends before the next mutation.
- Pops return owned values (`Option<Vec<u8>>`), because the element leaves
  the Store. `keys` also returns owned bytes so callers can keep mutating.
- Mutating methods take `&mut self`. The Store does no internal locking;
  callers wrap it in a `Mutex` or `RwLock` if they need to share it.
- Errors are `Result<_, Error>` with one `#[non_exhaustive]` enum
  (`WrongType`, `NotAnInteger`, `Overflow`). A failed command changes nothing.
- Deliberate Rust-flavoured deviations: single-key `del` returning a bool, no
  `count` on pops, and `set` returning `()`.

## Alternatives considered

- **Typed enum API** (e.g. `store.insert(key, Value::List(..))`) instead of a
  Redis-faithful command surface. Rejected: it makes existing Redis knowledge
  useless, exposes the value enum, and pushes type-matching onto every caller.
- **Internal locking** (`&self` methods behind a `Mutex`/`RwLock`) instead of
  `&mut self`. Rejected: it forces a locking strategy and overhead on
  single-threaded users, and borrowed reads cannot outlive a lock guard.
  `&mut self` lets callers choose.
- **Clone on read** (return owned copies) instead of borrowing. Rejected:
  it copies on every read. Borrowing is free, and the borrow checker already
  prevents use during mutation.

## Consequences

- Reads are zero-copy, but a returned slice blocks mutation of the Store
  while it is alive; callers copy if they need to hold data across writes.
- Sharing across threads is the caller's responsibility and choice.
- Users who know Redis can predict behaviour; differences are few and listed
  above.
- The crate stays dependency-free and small; new commands and `Error`
  variants can be added without breaking callers.
