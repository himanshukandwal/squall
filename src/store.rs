use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::time::Instant;

use crate::clock::{Clock, SystemClock};
use crate::value::Value;

/// An in-memory keyspace mapping binary-safe keys to strings, lists, hashes
/// or sets.
///
/// Commands are methods named after their Redis equivalents. The store does
/// no internal locking; wrap it in a `Mutex` or `RwLock` to share it.
///
/// ```
/// let store = squall::Store::new();
/// assert_eq!(store.len(), 0);
/// ```
pub struct Store {
    // Private so that every command goes through the helpers below: they are
    // the single place that reads, inserts into or removes from the Keyspace.
    map: HashMap<Vec<u8>, Value>,
    // Invariants: every Key in `deadlines` is in `map`, and `index` holds
    // exactly the entries of `deadlines`, ordered by deadline.
    deadlines: HashMap<Vec<u8>, Instant>,
    index: BTreeSet<(Instant, Vec<u8>)>,
    clock: Box<dyn Clock>,
}

/// Whether a Key with `deadline` is expired at `now`. The only place this
/// comparison is made.
pub(crate) fn is_expired(deadline: Instant, now: Instant) -> bool {
    deadline <= now
}

// Keyspace access helpers. Commands never touch `map`, `deadlines` or
// `index` directly.
//
// `&mut self` commands call `drain_expired` (or `drain_due`) first, so the
// other `&mut` helpers never see an Expired key. `lookup` is for `&self`
// reads, which never delete, so it filters Expired keys instead.
impl Store {
    /// Looks up `key` for reading. An Expired key reads as missing.
    pub(crate) fn lookup(&self, key: &[u8]) -> Option<&Value> {
        // With no TTL on this Key (or none at all), skip the clock.
        if !self.deadlines.is_empty()
            && let Some(&deadline) = self.deadlines.get(key)
            && is_expired(deadline, self.now())
        {
            return None;
        }
        self.map.get(key)
    }

    /// Looks up `key` for an in-place write that keeps the Key's metadata.
    ///
    /// The caller must drain first (`drain_expired` or `drain_due`): an
    /// Expired key would be returned as if live. Debug builds check this.
    pub(crate) fn lookup_mut(&mut self, key: &[u8]) -> Option<&mut Value> {
        self.debug_assert_drained(key);
        self.map.get_mut(key)
    }

    /// Looks up `key` for writing, first creating it with `create()` if it is
    /// missing. An existing Key is returned as is, whatever its type.
    ///
    /// The caller must drain first (`drain_expired` or `drain_due`): an
    /// Expired key would be returned as if live. Debug builds check this.
    pub(crate) fn lookup_or_create(
        &mut self,
        key: &[u8],
        create: impl FnOnce() -> Value,
    ) -> &mut Value {
        self.debug_assert_drained(key);
        self.map.entry(key.to_vec()).or_insert_with(create)
    }

    /// In debug builds, panics if `key` is Expired: the calling command did
    /// not drain first. Reads the clock only when `key` has a TTL; release
    /// builds do nothing.
    ///
    /// This reads the clock again after the command's drain. With a real
    /// clock, a Key that falls due between the drain's read and this one
    /// trips it falsely. That window is the drain itself plus a few
    /// instructions, so it takes a Key written right at its deadline. A
    /// `ManualClock` never moves within a command, so tests cannot hit it.
    fn debug_assert_drained(&self, key: &[u8]) {
        if cfg!(debug_assertions)
            && let Some(&deadline) = self.deadlines.get(key)
        {
            assert!(
                !is_expired(deadline, self.now()),
                "write lookup of an Expired key: caller must drain first"
            );
        }
    }

    /// Inserts `value` at `key`, replacing any existing Value of any type.
    /// The new Value has no TTL.
    pub(crate) fn put(&mut self, key: &[u8], value: Value) {
        self.clear_deadline(key);
        self.map.insert(key.to_vec(), value);
    }

    /// Removes `key` and its TTL; returns whether it existed. Used by `del`
    /// and by the automatic removal of emptied containers.
    pub(crate) fn remove_key(&mut self, key: &[u8]) -> bool {
        self.clear_deadline(key);
        self.map.remove(key).is_some()
    }

    /// Iterates over every live Key, in unspecified order. Reads the clock
    /// once, and not at all when no Key has a TTL.
    pub(crate) fn key_iter(&self) -> impl Iterator<Item = &Vec<u8>> {
        let now = (!self.index.is_empty()).then(|| self.now());
        self.map
            .keys()
            .filter(move |key| match (now, self.deadlines.get(*key)) {
                (Some(now), Some(&deadline)) => !is_expired(deadline, now),
                _ => true,
            })
    }

    /// Returns the number of live Keys: all Keys minus the due entries at the
    /// front of the deadline index. One look at the earliest deadline when
    /// none is due; no clock read when no Key has a TTL.
    pub(crate) fn key_count(&self) -> usize {
        if self.index.is_empty() {
            return self.map.len();
        }
        let now = self.now();
        let due = self
            .index
            .iter()
            .take_while(|(deadline, _)| is_expired(*deadline, now))
            .count();
        self.map.len() - due
    }

    /// Reads the Store's clock.
    pub(crate) fn now(&self) -> Instant {
        self.clock.now()
    }

    /// Returns the deadline of `key`, if it has a TTL. The Key may be
    /// Expired; the caller checks with [`is_expired`].
    pub(crate) fn deadline(&self, key: &[u8]) -> Option<Instant> {
        self.deadlines.get(key).copied()
    }

    /// Gives the existing `key` a TTL ending at `deadline`, replacing any
    /// earlier one.
    pub(crate) fn set_deadline(&mut self, key: &[u8], deadline: Instant) {
        debug_assert!(self.map.contains_key(key), "TTL on a missing Key");
        if let Some(old) = self.deadlines.insert(key.to_vec(), deadline) {
            self.index.remove(&(old, key.to_vec()));
        }
        self.index.insert((deadline, key.to_vec()));
    }

    /// Removes the TTL of `key`; returns whether it had one.
    pub(crate) fn clear_deadline(&mut self, key: &[u8]) -> bool {
        match self.deadlines.remove(key) {
            Some(old) => {
                self.index.remove(&(old, key.to_vec()));
                true
            }
            None => false,
        }
    }

    /// Removes every Key whose deadline has passed at `now`; returns how
    /// many were removed. One look at the earliest deadline when none is due.
    pub(crate) fn drain_due(&mut self, now: Instant) -> usize {
        let mut removed = 0;
        while let Some((deadline, _)) = self.index.first()
            && is_expired(*deadline, now)
        {
            let (_, key) = self.index.pop_first().expect("index is non-empty");
            self.deadlines.remove(&key);
            self.map.remove(&key);
            removed += 1;
        }
        removed
    }

    /// Like [`Store::drain_due`] at the current time, but skips reading the
    /// clock when no Key has a TTL.
    pub(crate) fn drain_expired(&mut self) -> usize {
        if self.index.is_empty() {
            return 0;
        }
        let now = self.now();
        self.drain_due(now)
    }
}

impl Store {
    /// Creates an empty store that reads time from the system's monotonic
    /// clock ([`SystemClock`](crate::SystemClock)). No Redis equivalent: a
    /// Redis database simply starts out empty.
    ///
    /// ```
    /// let store = squall::Store::new();
    /// assert_eq!(store.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self::with_clock(SystemClock)
    }

    /// Creates an empty store that reads time from `clock`, which decides
    /// when Keys with a TTL expire. No Redis equivalent.
    ///
    /// ```
    /// use std::time::Duration;
    /// use squall::{ManualClock, Store};
    ///
    /// let clock = ManualClock::new();
    /// let mut store = Store::with_clock(clock.clone());
    /// store.set("k", "v");
    /// store.expire("k", Duration::from_secs(1));
    /// clock.advance(Duration::from_secs(1));
    /// assert_eq!(store.get("k").unwrap(), None);
    /// ```
    pub fn with_clock(clock: impl Clock + 'static) -> Self {
        Self {
            map: HashMap::new(),
            deadlines: HashMap::new(),
            index: BTreeSet::new(),
            clock: Box::new(clock),
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("map", &self.map)
            .field("deadlines", &self.deadlines)
            .field("clock", &format_args!("<dyn Clock>"))
            .finish()
    }
}

// The write helpers' precondition is checked by `debug_assert!`, so these
// tests only make sense when debug assertions are on.
#[cfg(all(test, debug_assertions))]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::clock::ManualClock;

    /// A Store holding the string "k", whose TTL of 1s has run out, not yet
    /// drained.
    fn store_with_undrained_expired_key() -> Store {
        let clock = ManualClock::new();
        let mut store = Store::with_clock(clock.clone());
        store.set("k", "v");
        assert!(store.expire("k", Duration::from_secs(1)));
        clock.advance(Duration::from_secs(1));
        store
    }

    #[test]
    #[should_panic(expected = "caller must drain first")]
    fn lookup_mut_panics_on_an_undrained_expired_key() {
        let mut store = store_with_undrained_expired_key();
        store.lookup_mut(b"k");
    }

    #[test]
    #[should_panic(expected = "caller must drain first")]
    fn lookup_or_create_panics_on_an_undrained_expired_key() {
        let mut store = store_with_undrained_expired_key();
        store.lookup_or_create(b"k", || Value::String(Vec::new()));
    }

    #[test]
    fn write_lookups_after_a_drain_do_not_panic() {
        let mut store = store_with_undrained_expired_key();
        store.drain_expired();
        assert!(store.lookup_mut(b"k").is_none());
        store.lookup_or_create(b"k", || Value::String(Vec::new()));
    }
}
