//! Time sources for key expiry: the `Clock` trait, `SystemClock` and
//! `ManualClock`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A source of monotonic time that a [`Store`](crate::Store) reads to decide
/// when Keys with a TTL expire. No Redis equivalent: Redis always uses the
/// server's own clock.
///
/// Implement it to drive expiry from your own time source; it must be
/// `Send + Sync` so the Store stays shareable behind a `Mutex` or `RwLock`.
///
/// ```
/// use std::time::Instant;
/// use squall::{Clock, Store};
///
/// struct Frozen(Instant);
/// impl Clock for Frozen {
///     fn now(&self) -> Instant {
///         self.0
///     }
/// }
///
/// let mut store = Store::with_clock(Frozen(Instant::now()));
/// store.set("k", "v");
/// assert!(store.expire("k", std::time::Duration::from_secs(1)));
/// assert!(store.exists("k"));
/// ```
pub trait Clock: Send + Sync {
    /// Returns the current instant. Must never go backwards.
    ///
    /// ```
    /// use squall::{Clock, SystemClock};
    /// let a = SystemClock.now();
    /// assert!(SystemClock.now() >= a);
    /// ```
    fn now(&self) -> Instant;
}

/// The system's monotonic clock (`Instant::now()`), used by
/// [`Store::new`](crate::Store::new). Changing the wall clock never expires
/// or revives Keys. Redis: the server clock.
///
/// ```
/// use squall::{Store, SystemClock};
/// let mut store = Store::with_clock(SystemClock);
/// store.set("k", "v");
/// assert!(store.exists("k"));
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A clock that only moves when told to, for deterministic tests of TTLs.
/// No Redis equivalent.
///
/// It is a cheap handle: clones share one current time, so keep one clone
/// and give the other to the Store. It starts at the real `Instant::now()`
/// and only moves forward, with [`advance`](ManualClock::advance).
///
/// ```
/// use std::time::Duration;
/// use squall::{ManualClock, Store};
///
/// let clock = ManualClock::new();
/// let mut store = Store::with_clock(clock.clone());
/// store.set("k", "v");
/// store.expire("k", Duration::from_secs(10));
/// clock.advance(Duration::from_secs(10));
/// assert!(!store.exists("k"));
/// ```
#[derive(Debug, Clone)]
pub struct ManualClock {
    now: Arc<Mutex<Instant>>,
}

impl ManualClock {
    /// Creates a clock reading the real `Instant::now()`, which then stands
    /// still until advanced.
    ///
    /// ```
    /// use squall::{Clock, ManualClock};
    /// let clock = ManualClock::new();
    /// assert_eq!(clock.now(), clock.now());
    /// ```
    pub fn new() -> Self {
        Self {
            now: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Moves this clock, and every clone of it, forward by `by`.
    ///
    /// # Panics
    ///
    /// If the new time would overflow `Instant`.
    ///
    /// ```
    /// use std::time::Duration;
    /// use squall::{Clock, ManualClock};
    /// let clock = ManualClock::new();
    /// let other = clock.clone();
    /// let start = clock.now();
    /// other.advance(Duration::from_secs(5));
    /// assert_eq!(clock.now(), start + Duration::from_secs(5));
    /// ```
    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *now = now
            .checked_add(by)
            .expect("ManualClock advanced past the end of Instant");
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }
}
