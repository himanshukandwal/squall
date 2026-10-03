//! List commands.

use std::collections::VecDeque;
use std::collections::hash_map::Entry;

use crate::{Error, Store, value::Value};

/// Resolves a possibly negative Redis-style index against a list of `len`
/// elements. Returns the position before clamping (may be out of range).
fn resolve(index: i64, len: usize) -> i128 {
    let index = i128::from(index);
    if index < 0 { index + len as i128 } else { index }
}

impl Store {
    fn list(&self, key: &[u8]) -> Result<Option<&VecDeque<Vec<u8>>>, Error> {
        match self.map.get(key) {
            None => Ok(None),
            Some(Value::List(l)) => Ok(Some(l)),
            Some(_) => Err(Error::WrongType),
        }
    }

    fn push(
        &mut self,
        key: &[u8],
        values: impl IntoIterator<Item = impl AsRef<[u8]>>,
        front: bool,
    ) -> Result<usize, Error> {
        let list = match self.map.entry(key.to_vec()) {
            Entry::Occupied(e) => match e.into_mut() {
                Value::List(l) => l,
                _ => return Err(Error::WrongType),
            },
            Entry::Vacant(e) => match e.insert(Value::List(VecDeque::new())) {
                Value::List(l) => l,
                _ => unreachable!(),
            },
        };
        for v in values {
            let v = v.as_ref().to_vec();
            if front {
                list.push_front(v);
            } else {
                list.push_back(v);
            }
        }
        let len = list.len();
        // Pushing no values onto a missing key must not leave an empty list.
        if len == 0 {
            self.map.remove(key);
        }
        Ok(len)
    }

    fn pop(&mut self, key: &[u8], front: bool) -> Result<Option<Vec<u8>>, Error> {
        let list = match self.map.get_mut(key) {
            None => return Ok(None),
            Some(Value::List(l)) => l,
            Some(_) => return Err(Error::WrongType),
        };
        let item = if front { list.pop_front() } else { list.pop_back() };
        if list.is_empty() {
            self.map.remove(key);
        }
        Ok(item)
    }

    /// Pushes `values` onto the head of the list at `key`, creating it if
    /// missing, and returns the list length afterwards. Each value is pushed
    /// in turn, so the final order is the reverse of the arguments. Fails with
    /// [`Error::WrongType`] if the key holds a non-list. Redis: `LPUSH`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.lpush("l", ["a", "b", "c"]).unwrap(), 3);
    /// assert_eq!(store.lindex("l", 0).unwrap(), Some(&b"c"[..]));
    /// ```
    pub fn lpush(
        &mut self,
        key: impl AsRef<[u8]>,
        values: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<usize, Error> {
        self.push(key.as_ref(), values, true)
    }

    /// Pushes `values` onto the tail of the list at `key`, creating it if
    /// missing, and returns the list length afterwards. Argument order is
    /// preserved. Fails with [`Error::WrongType`] if the key holds a
    /// non-list. Redis: `RPUSH`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.rpush("l", ["a", "b", "c"]).unwrap(), 3);
    /// assert_eq!(store.lindex("l", 0).unwrap(), Some(&b"a"[..]));
    /// ```
    pub fn rpush(
        &mut self,
        key: impl AsRef<[u8]>,
        values: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<usize, Error> {
        self.push(key.as_ref(), values, false)
    }

    /// Removes and returns the head of the list at `key`, or `None` if the
    /// key is missing. Popping the last element deletes the key. Fails with
    /// [`Error::WrongType`] if the key holds a non-list. Redis: `LPOP`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.rpush("l", ["a", "b"]).unwrap();
    /// assert_eq!(store.lpop("l").unwrap(), Some(b"a".to_vec()));
    /// assert_eq!(store.lpop("missing").unwrap(), None);
    /// ```
    pub fn lpop(&mut self, key: impl AsRef<[u8]>) -> Result<Option<Vec<u8>>, Error> {
        self.pop(key.as_ref(), true)
    }

    /// Removes and returns the tail of the list at `key`, or `None` if the
    /// key is missing. Popping the last element deletes the key. Fails with
    /// [`Error::WrongType`] if the key holds a non-list. Redis: `RPOP`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.rpush("l", ["a", "b"]).unwrap();
    /// assert_eq!(store.rpop("l").unwrap(), Some(b"b".to_vec()));
    /// assert_eq!(store.rpop("missing").unwrap(), None);
    /// ```
    pub fn rpop(&mut self, key: impl AsRef<[u8]>) -> Result<Option<Vec<u8>>, Error> {
        self.pop(key.as_ref(), false)
    }

    /// Returns the length of the list at `key`, or 0 if the key is missing.
    /// Fails with [`Error::WrongType`] if the key holds a non-list.
    /// Redis: `LLEN`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.llen("l").unwrap(), 0);
    /// store.rpush("l", ["a", "b"]).unwrap();
    /// assert_eq!(store.llen("l").unwrap(), 2);
    /// ```
    pub fn llen(&self, key: impl AsRef<[u8]>) -> Result<usize, Error> {
        Ok(self.list(key.as_ref())?.map_or(0, VecDeque::len))
    }

    /// Returns the element at `index` of the list at `key`. A negative index
    /// counts from the end (`-1` is the last element). Out of range or a
    /// missing key gives `None`. Fails with [`Error::WrongType`] if the key
    /// holds a non-list. Redis: `LINDEX`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.rpush("l", ["a", "b", "c"]).unwrap();
    /// assert_eq!(store.lindex("l", 1).unwrap(), Some(&b"b"[..]));
    /// assert_eq!(store.lindex("l", -1).unwrap(), Some(&b"c"[..]));
    /// assert_eq!(store.lindex("l", 3).unwrap(), None);
    /// ```
    pub fn lindex(&self, key: impl AsRef<[u8]>, index: i64) -> Result<Option<&[u8]>, Error> {
        let Some(list) = self.list(key.as_ref())? else {
            return Ok(None);
        };
        let pos = resolve(index, list.len());
        let Ok(pos) = usize::try_from(pos) else {
            return Ok(None);
        };
        Ok(list.get(pos).map(Vec::as_slice))
    }

    /// Returns the elements of the list at `key` between `start` and `stop`,
    /// both inclusive. Negative indices count from the end and out-of-range
    /// bounds are clamped. A missing key or an empty window gives an empty
    /// result. Fails with [`Error::WrongType`] if the key holds a non-list.
    /// Redis: `LRANGE`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.rpush("l", ["a", "b", "c"]).unwrap();
    /// assert_eq!(store.lrange("l", 0, 1).unwrap(), vec![&b"a"[..], &b"b"[..]]);
    /// assert_eq!(store.lrange("l", -2, 100).unwrap(), vec![&b"b"[..], &b"c"[..]]);
    /// assert!(store.lrange("l", 2, 1).unwrap().is_empty());
    /// ```
    pub fn lrange(
        &self,
        key: impl AsRef<[u8]>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<&[u8]>, Error> {
        let Some(list) = self.list(key.as_ref())? else {
            return Ok(Vec::new());
        };
        let len = list.len() as i128;
        let start = resolve(start, list.len()).max(0);
        let stop = resolve(stop, list.len()).min(len - 1);
        if start > stop {
            return Ok(Vec::new());
        }
        Ok(list
            .range(start as usize..=stop as usize)
            .map(Vec::as_slice)
            .collect())
    }
}
