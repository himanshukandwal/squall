//! Hash commands.

use crate::{Error, Store, value::Value};
use std::collections::HashMap;

type Hash = HashMap<Vec<u8>, Vec<u8>>;

impl Store {
    fn hash(&self, key: &[u8]) -> Result<Option<&Hash>, Error> {
        match self.map.get(key) {
            None => Ok(None),
            Some(Value::Hash(h)) => Ok(Some(h)),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Sets `field` in the hash at `key`, creating the hash if missing.
    /// Returns `true` if the field is new, `false` if it overwrote an
    /// existing field. Fails with [`Error::WrongType`] if the key holds a
    /// non-hash. Redis: `HSET` (single field).
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert!(store.hset("h", "f", "1").unwrap());
    /// assert!(!store.hset("h", "f", "2").unwrap());
    /// assert_eq!(store.hget("h", "f").unwrap(), Some(&b"2"[..]));
    /// ```
    pub fn hset(
        &mut self,
        key: impl AsRef<[u8]>,
        field: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<bool, Error> {
        let entry = self
            .map
            .entry(key.as_ref().to_vec())
            .or_insert_with(|| Value::Hash(Hash::new()));
        match entry {
            Value::Hash(h) => Ok(h
                .insert(field.as_ref().to_vec(), value.as_ref().to_vec())
                .is_none()),
            _ => Err(Error::WrongType),
        }
    }

    /// Returns the value of `field`, or `None` if the key or field is
    /// missing. Fails with [`Error::WrongType`] if the key holds a non-hash.
    /// Redis: `HGET`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.hget("h", "f").unwrap(), None);
    /// store.hset("h", "f", "v").unwrap();
    /// assert_eq!(store.hget("h", "f").unwrap(), Some(&b"v"[..]));
    /// ```
    pub fn hget(
        &self,
        key: impl AsRef<[u8]>,
        field: impl AsRef<[u8]>,
    ) -> Result<Option<&[u8]>, Error> {
        Ok(self
            .hash(key.as_ref())?
            .and_then(|h| h.get(field.as_ref()))
            .map(|v| v.as_slice()))
    }

    /// Removes the given fields and returns how many were actually removed.
    /// Removing the last field deletes the key. Fails with
    /// [`Error::WrongType`] if the key holds a non-hash. Redis: `HDEL`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.hset("h", "a", "1").unwrap();
    /// store.hset("h", "b", "2").unwrap();
    /// assert_eq!(store.hdel("h", ["a", "nope"]).unwrap(), 1);
    /// assert_eq!(store.hdel("h", ["b"]).unwrap(), 1);
    /// assert!(!store.exists("h"));
    /// ```
    pub fn hdel<I>(&mut self, key: impl AsRef<[u8]>, fields: I) -> Result<usize, Error>
    where
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
    {
        let key = key.as_ref();
        let (removed, empty) = match self.map.get_mut(key) {
            None => return Ok(0),
            Some(Value::Hash(h)) => {
                let removed = fields
                    .into_iter()
                    .filter(|f| h.remove(f.as_ref()).is_some())
                    .count();
                (removed, h.is_empty())
            }
            Some(_) => return Err(Error::WrongType),
        };
        if empty {
            self.map.remove(key);
        }
        Ok(removed)
    }

    /// Returns whether `field` exists in the hash at `key` (`false` for a
    /// missing key). Fails with [`Error::WrongType`] if the key holds a
    /// non-hash. Redis: `HEXISTS`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert!(!store.hexists("h", "f").unwrap());
    /// store.hset("h", "f", "v").unwrap();
    /// assert!(store.hexists("h", "f").unwrap());
    /// ```
    pub fn hexists(&self, key: impl AsRef<[u8]>, field: impl AsRef<[u8]>) -> Result<bool, Error> {
        Ok(self
            .hash(key.as_ref())?
            .is_some_and(|h| h.contains_key(field.as_ref())))
    }

    /// Returns the number of fields in the hash at `key` (0 for a missing
    /// key). Fails with [`Error::WrongType`] if the key holds a non-hash.
    /// Redis: `HLEN`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.hlen("h").unwrap(), 0);
    /// store.hset("h", "f", "v").unwrap();
    /// assert_eq!(store.hlen("h").unwrap(), 1);
    /// ```
    pub fn hlen(&self, key: impl AsRef<[u8]>) -> Result<usize, Error> {
        Ok(self.hash(key.as_ref())?.map_or(0, |h| h.len()))
    }

    /// Returns all `(field, value)` pairs, in unspecified order; empty for a
    /// missing key. Fails with [`Error::WrongType`] if the key holds a
    /// non-hash. Redis: `HGETALL`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.hset("h", "f", "v").unwrap();
    /// assert_eq!(store.hgetall("h").unwrap(), vec![(&b"f"[..], &b"v"[..])]);
    /// ```
    pub fn hgetall(&self, key: impl AsRef<[u8]>) -> Result<Vec<(&[u8], &[u8])>, Error> {
        Ok(self.hash(key.as_ref())?.map_or_else(Vec::new, |h| {
            h.iter()
                .map(|(f, v)| (f.as_slice(), v.as_slice()))
                .collect()
        }))
    }

    /// Returns all field names, in unspecified order; empty for a missing
    /// key. Fails with [`Error::WrongType`] if the key holds a non-hash.
    /// Redis: `HKEYS`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.hset("h", "f", "v").unwrap();
    /// assert_eq!(store.hkeys("h").unwrap(), vec![&b"f"[..]]);
    /// ```
    pub fn hkeys(&self, key: impl AsRef<[u8]>) -> Result<Vec<&[u8]>, Error> {
        Ok(self
            .hash(key.as_ref())?
            .map_or_else(Vec::new, |h| h.keys().map(|f| f.as_slice()).collect()))
    }
}
