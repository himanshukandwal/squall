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
once and holds exactly one Value. An emptied list, hash or set is removed
from the Keyspace, so a Keyspace never contains an empty container.

**Key**
The name a Value is stored under. Arbitrary bytes (binary-safe). Callers may
pass `&str`, `String`, `&[u8]` or `Vec<u8>`.

**Value**
What a Key holds: one datum of exactly one Type. The internal value enum is
never exposed. Strings, list elements, hash fields and values, and set
members are all arbitrary bytes.

**Type**
Which of the four kinds a Value is. A command run on a Key holding a
different Type fails with `WrongType`. The one exception is `set`, which
overwrites a Key of any Type.

### The four types

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

## Behaviour terms

**Missing key**
A Key not in the Keyspace. Reads treat it as empty (`None`, an empty result,
or 0). Creating writes (`append`, pushes, `hset`, `sadd`, counters) create it.

**WrongType / NotAnInteger / Overflow**
The variants of the single `Error` enum (`#[non_exhaustive]`). A command that
returns an error has made no change to the Store.

## Not in scope

No server or network protocol, persistence, or key expiry. See
`docs/adr/` for design decisions.
