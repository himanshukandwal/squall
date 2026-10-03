use std::collections::HashMap;

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
#[derive(Debug, Default)]
pub struct Store {
    pub(crate) map: HashMap<Vec<u8>, Value>,
}

impl Store {
    /// Creates an empty store. No Redis equivalent: a Redis database simply
    /// starts out empty.
    ///
    /// ```
    /// let store = squall::Store::new();
    /// assert_eq!(store.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }
}
