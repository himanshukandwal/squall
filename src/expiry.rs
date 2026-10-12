//! Key expiry commands: `expire`, `ttl`, `persist` and `purge_expired`.

use std::time::Duration;

use crate::{Store, store::is_expired};

/// The time to live of a Key, as returned by [`Store::ttl`].
///
/// ```
/// use std::time::Duration;
/// use squall::{ManualClock, Store, Ttl};
///
/// let mut store = Store::with_clock(ManualClock::new());
/// assert_eq!(store.ttl("k"), Ttl::Missing);
/// store.set("k", "v");
/// assert_eq!(store.ttl("k"), Ttl::Persistent);
/// store.expire("k", Duration::from_secs(5));
/// assert_eq!(store.ttl("k"), Ttl::Remaining(Duration::from_secs(5)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ttl {
    /// The Key is missing or Expired. Redis: `-2`.
    Missing,
    /// The Key exists and has no TTL. Redis: `-1`.
    Persistent,
    /// The Key expires after exactly this much more time.
    Remaining(Duration),
}

impl Store {
    /// Gives the existing `key` a TTL of `ttl`, replacing any earlier TTL,
    /// and returns `true`; returns `false` and creates nothing if the key is
    /// missing. Redis: `PEXPIRE` (no flags).
    ///
    /// `Duration::ZERO` deletes the key at once. A `ttl` so large that the
    /// deadline would overflow `Instant` leaves the key with no TTL (it never
    /// expires).
    ///
    /// ```
    /// use std::time::Duration;
    /// use squall::{ManualClock, Store};
    ///
    /// let clock = ManualClock::new();
    /// let mut store = Store::with_clock(clock.clone());
    /// assert!(!store.expire("k", Duration::from_secs(10)));
    /// store.set("k", "v");
    /// assert!(store.expire("k", Duration::from_secs(10)));
    /// clock.advance(Duration::from_secs(10));
    /// assert!(!store.exists("k"));
    /// ```
    pub fn expire(&mut self, key: impl AsRef<[u8]>, ttl: Duration) -> bool {
        let key = key.as_ref();
        let now = self.now();
        self.drain_due(now);
        if self.lookup_mut(key).is_none() {
            return false;
        }
        if ttl.is_zero() {
            self.remove_key(key);
            return true;
        }
        match now.checked_add(ttl) {
            Some(deadline) => self.set_deadline(key, deadline),
            None => {
                self.clear_deadline(key);
            }
        }
        true
    }

    /// Returns the time left before `key` expires: [`Ttl::Remaining`] with
    /// the exact time, [`Ttl::Persistent`] if it has no TTL, or
    /// [`Ttl::Missing`] if it is missing or Expired. Redis: `PTTL`.
    ///
    /// ```
    /// use std::time::Duration;
    /// use squall::{ManualClock, Store, Ttl};
    ///
    /// let clock = ManualClock::new();
    /// let mut store = Store::with_clock(clock.clone());
    /// store.set("k", "v");
    /// store.expire("k", Duration::from_secs(10));
    /// clock.advance(Duration::from_secs(4));
    /// assert_eq!(store.ttl("k"), Ttl::Remaining(Duration::from_secs(6)));
    /// ```
    pub fn ttl(&self, key: impl AsRef<[u8]>) -> Ttl {
        let key = key.as_ref();
        match self.deadline(key) {
            Some(deadline) => {
                let now = self.now();
                if is_expired(deadline, now) {
                    Ttl::Missing
                } else {
                    Ttl::Remaining(deadline - now)
                }
            }
            // No TTL, so `lookup` does not read the clock.
            None if self.lookup(key).is_some() => Ttl::Persistent,
            None => Ttl::Missing,
        }
    }

    /// Removes the TTL of `key`; returns `true` if it had one, `false` if the
    /// key is missing or has no TTL. Redis: `PERSIST`.
    ///
    /// ```
    /// use std::time::Duration;
    /// use squall::{Store, Ttl};
    ///
    /// let mut store = Store::new();
    /// store.set("k", "v");
    /// assert!(!store.persist("k"));
    /// store.expire("k", Duration::from_secs(10));
    /// assert!(store.persist("k"));
    /// assert_eq!(store.ttl("k"), Ttl::Persistent);
    /// ```
    pub fn persist(&mut self, key: impl AsRef<[u8]>) -> bool {
        self.drain_expired();
        self.clear_deadline(key.as_ref())
    }

    /// Removes every Expired key from memory now and returns how many were
    /// removed. Every other mutating command already does this first, so
    /// this is only needed to reclaim memory during read-only phases. No
    /// Redis equivalent (Redis expires keys in the background).
    ///
    /// ```
    /// use std::time::Duration;
    /// use squall::{ManualClock, Store};
    ///
    /// let clock = ManualClock::new();
    /// let mut store = Store::with_clock(clock.clone());
    /// store.set("a", "1");
    /// store.set("b", "2");
    /// store.expire("a", Duration::from_secs(1));
    /// clock.advance(Duration::from_secs(1));
    /// assert_eq!(store.purge_expired(), 1);
    /// assert_eq!(store.purge_expired(), 0);
    /// ```
    pub fn purge_expired(&mut self) -> usize {
        self.drain_expired()
    }
}
