//! Keyspace commands and basic string `set` / `get`.

use crate::{Error, Store, value::Value};

impl Store {
    /// Sets `key` to a string `value`, overwriting any existing value of any
    /// type. Redis: `SET`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.set("name", "squall");
    /// assert_eq!(store.get("name").unwrap(), Some(&b"squall"[..]));
    /// ```
    pub fn set(&mut self, key: impl AsRef<[u8]>, value: impl AsRef<[u8]>) {
        self.map
            .insert(key.as_ref().to_vec(), Value::String(value.as_ref().to_vec()));
    }

    /// Returns the string stored at `key`, or `None` if the key is missing.
    /// Fails with [`Error::WrongType`] if the key holds a non-string.
    /// Redis: `GET`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.get("k").unwrap(), None);
    /// store.set("k", "v");
    /// assert_eq!(store.get("k").unwrap(), Some(&b"v"[..]));
    /// ```
    pub fn get(&self, key: impl AsRef<[u8]>) -> Result<Option<&[u8]>, Error> {
        match self.map.get(key.as_ref()) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s)),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Removes `key` of any type; returns whether it existed. Redis: `DEL`
    /// (single key).
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.set("k", "v");
    /// assert!(store.del("k"));
    /// assert!(!store.del("k"));
    /// ```
    pub fn del(&mut self, key: impl AsRef<[u8]>) -> bool {
        self.map.remove(key.as_ref()).is_some()
    }

    /// Returns whether `key` exists, regardless of its type. Redis: `EXISTS`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert!(!store.exists("k"));
    /// store.set("k", "v");
    /// assert!(store.exists("k"));
    /// ```
    pub fn exists(&self, key: impl AsRef<[u8]>) -> bool {
        self.map.contains_key(key.as_ref())
    }

    /// Returns all keys as owned byte vectors, in unspecified order. Redis:
    /// `KEYS *` (no glob support).
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.set("a", "1");
    /// assert_eq!(store.keys(), vec![b"a".to_vec()]);
    /// ```
    pub fn keys(&self) -> Vec<Vec<u8>> {
        self.map.keys().cloned().collect()
    }

    /// Returns the number of keys in the store. Redis: `DBSIZE`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.set("a", "1");
    /// assert_eq!(store.len(), 1);
    /// ```
    // The spec's v1 command set has no `is_empty`, so don't add one.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
