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
    // Private so that every command goes through the helpers below: they are
    // the single place that reads, inserts into or removes from the Keyspace.
    map: HashMap<Vec<u8>, Value>,
}

// Keyspace access helpers. Commands never touch `map` directly.
impl Store {
    /// Looks up `key` for reading.
    pub(crate) fn lookup(&self, key: &[u8]) -> Option<&Value> {
        self.map.get(key)
    }

    /// Looks up `key` for an in-place write that keeps the Key's metadata.
    pub(crate) fn lookup_mut(&mut self, key: &[u8]) -> Option<&mut Value> {
        self.map.get_mut(key)
    }

    /// Looks up `key` for writing, first creating it with `create()` if it is
    /// missing. An existing Key is returned as is, whatever its type.
    pub(crate) fn lookup_or_create(
        &mut self,
        key: &[u8],
        create: impl FnOnce() -> Value,
    ) -> &mut Value {
        self.map.entry(key.to_vec()).or_insert_with(create)
    }

    /// Inserts `value` at `key`, replacing any existing Value of any type.
    pub(crate) fn put(&mut self, key: &[u8], value: Value) {
        self.map.insert(key.to_vec(), value);
    }

    /// Removes `key`; returns whether it existed. Used by `del` and by the
    /// automatic removal of emptied containers.
    pub(crate) fn remove_key(&mut self, key: &[u8]) -> bool {
        self.map.remove(key).is_some()
    }

    /// Iterates over every Key, in unspecified order.
    pub(crate) fn key_iter(&self) -> impl Iterator<Item = &Vec<u8>> {
        self.map.keys()
    }

    /// Returns the number of Keys.
    pub(crate) fn key_count(&self) -> usize {
        self.map.len()
    }
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
