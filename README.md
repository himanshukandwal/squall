# squall

A small in-memory key-value store in Rust, inspired by the data types of
[Redis](https://redis.io). A *squall* is a sudden, short storm: small and
fast.

## What it is

A library of data structures you use from Rust code:

- a **keyspace**: set, get, delete, check and list keys;
- **strings**, **lists**, **hashes** and **sets** as values.

## What it isn't

No server or network protocol, no persistence to disk, and no key expiry.
It's the data-structure layer only.

## Build and test

```
cargo build
cargo test
```

## How it's built

squall is being built with [Marut](https://github.com/himanshukandwal/marut),
a Claude Code plugin for spec-first, multi-agent work, as a real-world test
of that plugin.
