use squall::{Bound, Clock, Error, ManualClock, Store};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const MS: Duration = Duration::from_millis(1);
const SEC: Duration = Duration::from_secs(1);

fn store() -> (Store, ManualClock) {
    let clock = ManualClock::new();
    (Store::with_clock(clock.clone()), clock)
}

const TYPED_KEYS: [&str; 5] = ["str", "list", "hash", "set", "zset"];

/// One Key of each Type, every one with a TTL of `ttl`.
fn store_with_each_type(ttl: Duration) -> (Store, ManualClock) {
    let (mut s, clock) = store();
    s.set("str", "v");
    s.rpush("list", ["a"]).unwrap();
    s.hset("hash", "f", "v").unwrap();
    s.sadd("set", ["m"]).unwrap();
    s.zadd("zset", [(1.0, "m")]).unwrap();
    for k in TYPED_KEYS {
        assert!(s.expire(k, ttl));
    }
    (s, clock)
}

fn all() -> (Bound, Bound) {
    (
        Bound::Inclusive(f64::NEG_INFINITY),
        Bound::Inclusive(f64::INFINITY),
    )
}

/// Every read family returns the missing result for `key`, never `WrongType`.
fn assert_reads_missing(s: &Store, key: &str) {
    let (min, max) = all();
    assert_eq!(s.get(key).unwrap(), None, "get {key}");
    assert!(!s.exists(key), "exists {key}");
    assert_eq!(s.strlen(key).unwrap(), 0, "strlen {key}");
    assert_eq!(s.llen(key).unwrap(), 0, "llen {key}");
    assert_eq!(s.lindex(key, 0).unwrap(), None, "lindex {key}");
    assert!(s.lrange(key, 0, -1).unwrap().is_empty(), "lrange {key}");
    assert_eq!(s.hget(key, "f").unwrap(), None, "hget {key}");
    assert!(!s.hexists(key, "f").unwrap(), "hexists {key}");
    assert_eq!(s.hlen(key).unwrap(), 0, "hlen {key}");
    assert!(s.hkeys(key).unwrap().is_empty(), "hkeys {key}");
    assert!(s.hgetall(key).unwrap().is_empty(), "hgetall {key}");
    assert!(!s.sismember(key, "m").unwrap(), "sismember {key}");
    assert_eq!(s.scard(key).unwrap(), 0, "scard {key}");
    assert!(s.smembers(key).unwrap().is_empty(), "smembers {key}");
    assert_eq!(s.zscore(key, "m").unwrap(), None, "zscore {key}");
    assert_eq!(s.zcard(key).unwrap(), 0, "zcard {key}");
    assert!(s.zrange(key, 0, -1).unwrap().is_empty(), "zrange {key}");
    assert!(
        s.zrange_withscores(key, 0, -1).unwrap().is_empty(),
        "zrange_withscores {key}"
    );
    assert!(
        s.zrangebyscore(key, min, max).unwrap().is_empty(),
        "zrangebyscore {key}"
    );
    assert!(
        s.zrangebyscore_withscores(key, min, max)
            .unwrap()
            .is_empty(),
        "zrangebyscore_withscores {key}"
    );
}

/// The Key is live: its own Type's read sees it and other Types' reads fail
/// with `WrongType`.
fn assert_live(s: &Store, key: &str) {
    assert!(s.exists(key), "exists {key}");
    let (min, max) = all();
    let own_or_wrongtype = |own: &str, got: Result<bool, Error>| {
        if key == own {
            assert!(got.unwrap(), "{key} should be visible to its own reads");
        } else {
            assert_eq!(got, Err(Error::WrongType), "{key} read as {own}");
        }
    };
    own_or_wrongtype("str", s.strlen(key).map(|n| n == 1));
    own_or_wrongtype("list", s.llen(key).map(|n| n == 1));
    own_or_wrongtype("list", s.lrange(key, 0, -1).map(|v| v.len() == 1));
    own_or_wrongtype("hash", s.hexists(key, "f"));
    own_or_wrongtype("hash", s.hgetall(key).map(|v| v.len() == 1));
    own_or_wrongtype("set", s.sismember(key, "m"));
    own_or_wrongtype("set", s.smembers(key).map(|v| v.len() == 1));
    own_or_wrongtype("zset", s.zscore(key, "m").map(|v| v == Some(1.0)));
    own_or_wrongtype(
        "zset",
        s.zrangebyscore_withscores(key, min, max)
            .map(|v| v.len() == 1),
    );
}

fn set_of(keys: &[&str]) -> BTreeSet<Vec<u8>> {
    keys.iter().map(|k| k.as_bytes().to_vec()).collect()
}

fn keys(s: &Store) -> BTreeSet<Vec<u8>> {
    s.keys().into_iter().collect()
}

fn scan(s: &Store, pat: &str) -> BTreeSet<Vec<u8>> {
    s.scan(pat).into_iter().collect()
}

#[test]
fn expired_key_of_each_type_reads_as_missing_to_every_read_family() {
    let (s, clock) = store_with_each_type(SEC);
    clock.advance(SEC);
    // Read-only phase: no `&mut` command or purge has run since expiry.
    for k in TYPED_KEYS {
        assert_reads_missing(&s, k);
    }
}

#[test]
fn every_read_sees_key_just_before_deadline_and_misses_it_exactly_at_it() {
    let (s, clock) = store_with_each_type(10 * SEC);
    clock.advance(10 * SEC - MS);
    for k in TYPED_KEYS {
        assert_live(&s, k);
    }
    clock.advance(MS);
    for k in TYPED_KEYS {
        assert_reads_missing(&s, k);
    }
}

#[test]
fn len_is_exact_with_live_persistent_and_expired_keys() {
    let (mut s, clock) = store();
    s.set("persistent", "v");
    s.set("short", "v");
    s.set("long", "v");
    s.rpush("short-list", ["a"]).unwrap();
    s.expire("short", SEC);
    s.expire("short-list", SEC);
    s.expire("long", 10 * SEC);
    assert_eq!(s.len(), 4);
    clock.advance(SEC - MS);
    assert_eq!(s.len(), 4);
    clock.advance(MS);
    assert_eq!(s.len(), 2);
    clock.advance(9 * SEC);
    assert_eq!(s.len(), 1);
}

#[test]
fn len_is_unchanged_for_a_store_without_ttls() {
    let (mut s, clock) = store();
    s.set("a", "1");
    s.rpush("b", ["x"]).unwrap();
    clock.advance(1000 * SEC);
    assert_eq!(s.len(), 2);
}

#[test]
fn keys_and_scan_omit_expired_keys_and_keep_live_ones() {
    let (mut s, clock) = store();
    s.set("user:persistent", "v");
    s.set("user:long", "v");
    s.set("user:short", "v");
    s.sadd("other:short", ["m"]).unwrap();
    s.expire("user:long", 10 * SEC);
    s.expire("user:short", SEC);
    s.expire("other:short", SEC);
    clock.advance(SEC - MS);
    assert_eq!(
        keys(&s),
        set_of(&["user:persistent", "user:long", "user:short", "other:short"])
    );
    assert_eq!(
        scan(&s, "user:*"),
        set_of(&["user:persistent", "user:long", "user:short"])
    );
    clock.advance(MS);
    assert_eq!(keys(&s), set_of(&["user:persistent", "user:long"]));
    assert_eq!(
        scan(&s, "user:*"),
        set_of(&["user:persistent", "user:long"])
    );
    assert!(scan(&s, "other:*").is_empty());
}

/// A clock that counts how often it is read.
#[derive(Clone)]
struct CountingClock {
    start: Instant,
    reads: Arc<AtomicUsize>,
}

impl Clock for CountingClock {
    fn now(&self) -> Instant {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.start
    }
}

impl CountingClock {
    fn take(&self) -> usize {
        self.reads.swap(0, Ordering::SeqCst)
    }
}

#[test]
fn len_keys_and_scan_skip_the_clock_without_ttls_and_read_it_once_with() {
    let clock = CountingClock {
        start: Instant::now(),
        reads: Arc::new(AtomicUsize::new(0)),
    };
    let mut s = Store::with_clock(clock.clone());
    for k in ["a", "b", "c"] {
        s.set(k, "v");
    }
    clock.take();
    s.len();
    s.keys();
    s.scan("*");
    assert_eq!(clock.take(), 0);

    s.expire("a", SEC);
    s.expire("b", SEC);
    clock.take();
    s.len();
    assert!(clock.take() <= 1, "len");
    s.keys();
    assert!(clock.take() <= 1, "keys");
    s.scan("*");
    assert!(clock.take() <= 1, "scan");
}
