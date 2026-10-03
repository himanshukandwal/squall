//! Set commands.

use std::collections::HashSet;

use crate::{Error, Store, value::Value};

impl Store {
    /// Adds `members` to the set at `key`, creating it if missing, and
    /// returns how many were newly added. Duplicates are stored once. Fails
    /// with [`Error::WrongType`] if the key holds a non-set. Redis: `SADD`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.sadd("k", ["a", "b", "a"]).unwrap(), 2);
    /// assert_eq!(store.sadd("k", ["b", "c"]).unwrap(), 1);
    /// ```
    pub fn sadd<M: AsRef<[u8]>>(
        &mut self,
        key: impl AsRef<[u8]>,
        members: impl IntoIterator<Item = M>,
    ) -> Result<usize, Error> {
        let key = key.as_ref();
        if let Some(v) = self.map.get(key)
            && !matches!(v, Value::Set(_))
        {
            return Err(Error::WrongType);
        }
        let mut members = members.into_iter().peekable();
        if members.peek().is_none() {
            return Ok(0);
        }
        let Value::Set(set) = self
            .map
            .entry(key.to_vec())
            .or_insert_with(|| Value::Set(HashSet::new()))
        else {
            unreachable!("type checked above");
        };
        Ok(members
            .filter(|m| set.insert(m.as_ref().to_vec()))
            .count())
    }

    /// Removes `members` from the set at `key` and returns how many were
    /// actually removed. Removing the last member deletes the key. Fails with
    /// [`Error::WrongType`] if the key holds a non-set. Redis: `SREM`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.sadd("k", ["a", "b"]).unwrap();
    /// assert_eq!(store.srem("k", ["a", "zzz"]).unwrap(), 1);
    /// assert_eq!(store.srem("k", ["b"]).unwrap(), 1);
    /// assert!(!store.exists("k"));
    /// ```
    pub fn srem<M: AsRef<[u8]>>(
        &mut self,
        key: impl AsRef<[u8]>,
        members: impl IntoIterator<Item = M>,
    ) -> Result<usize, Error> {
        let key = key.as_ref();
        let (removed, now_empty) = match self.map.get_mut(key) {
            None => return Ok(0),
            Some(Value::Set(set)) => {
                let removed = members
                    .into_iter()
                    .filter(|m| set.remove(m.as_ref()))
                    .count();
                (removed, set.is_empty())
            }
            Some(_) => return Err(Error::WrongType),
        };
        if now_empty {
            self.map.remove(key);
        }
        Ok(removed)
    }

    /// Returns whether `member` is in the set at `key`; false for a missing
    /// key. Fails with [`Error::WrongType`] if the key holds a non-set.
    /// Redis: `SISMEMBER`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.sadd("k", ["a"]).unwrap();
    /// assert!(store.sismember("k", "a").unwrap());
    /// assert!(!store.sismember("k", "b").unwrap());
    /// ```
    pub fn sismember(
        &self,
        key: impl AsRef<[u8]>,
        member: impl AsRef<[u8]>,
    ) -> Result<bool, Error> {
        match self.map.get(key.as_ref()) {
            None => Ok(false),
            Some(Value::Set(set)) => Ok(set.contains(member.as_ref())),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Returns the number of members in the set at `key`; 0 for a missing
    /// key. Fails with [`Error::WrongType`] if the key holds a non-set.
    /// Redis: `SCARD`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.scard("k").unwrap(), 0);
    /// store.sadd("k", ["a", "b"]).unwrap();
    /// assert_eq!(store.scard("k").unwrap(), 2);
    /// ```
    pub fn scard(&self, key: impl AsRef<[u8]>) -> Result<usize, Error> {
        match self.map.get(key.as_ref()) {
            None => Ok(0),
            Some(Value::Set(set)) => Ok(set.len()),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Returns all members of the set at `key` in unspecified order; empty
    /// for a missing key. Fails with [`Error::WrongType`] if the key holds a
    /// non-set. Redis: `SMEMBERS`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.sadd("k", ["a", "b"]).unwrap();
    /// let mut members = store.smembers("k").unwrap();
    /// members.sort();
    /// assert_eq!(members, vec![&b"a"[..], &b"b"[..]]);
    /// ```
    pub fn smembers(&self, key: impl AsRef<[u8]>) -> Result<Vec<&[u8]>, Error> {
        match self.map.get(key.as_ref()) {
            None => Ok(Vec::new()),
            Some(Value::Set(set)) => Ok(set.iter().map(|m| m.as_slice()).collect()),
            Some(_) => Err(Error::WrongType),
        }
    }
}
