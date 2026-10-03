//! String commands: `append`, `strlen` and integer counters.

use crate::{Error, Store, value::Value};

/// Parses a stored string as a decimal `i64`, like Redis: optional leading
/// `-`, then digits only. No whitespace or `+`, and no leading zeros: after
/// the optional `-` the first digit must be 1-9, unless the whole string is
/// exactly `0` (so `007`, `00`, `-0` and `-007` are rejected).
fn parse_i64(bytes: &[u8]) -> Result<i64, Error> {
    let digits = bytes.strip_prefix(b"-").unwrap_or(bytes);
    if digits.first() == Some(&b'0') && (digits.len() > 1 || bytes.len() > 1) {
        return Err(Error::NotAnInteger);
    }
    if bytes.first() == Some(&b'+') {
        return Err(Error::NotAnInteger);
    }
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or(Error::NotAnInteger)
}

impl Store {
    /// Appends `value` to the string at `key`, creating the key if missing,
    /// and returns the new length in bytes. Fails with [`Error::WrongType`]
    /// if the key holds a non-string. Redis: `APPEND`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.append("k", "ab").unwrap(), 2);
    /// assert_eq!(store.append("k", "cd").unwrap(), 4);
    /// assert_eq!(store.get("k").unwrap(), Some(&b"abcd"[..]));
    /// ```
    pub fn append(
        &mut self,
        key: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<usize, Error> {
        match self.map.get_mut(key.as_ref()) {
            None => {
                let value = value.as_ref().to_vec();
                let len = value.len();
                self.map.insert(key.as_ref().to_vec(), Value::String(value));
                Ok(len)
            }
            Some(Value::String(s)) => {
                s.extend_from_slice(value.as_ref());
                Ok(s.len())
            }
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Returns the byte length of the string at `key`, or 0 if the key is
    /// missing. Fails with [`Error::WrongType`] if the key holds a
    /// non-string. Redis: `STRLEN`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.strlen("k").unwrap(), 0);
    /// store.set("k", "hello");
    /// assert_eq!(store.strlen("k").unwrap(), 5);
    /// ```
    pub fn strlen(&self, key: impl AsRef<[u8]>) -> Result<usize, Error> {
        match self.map.get(key.as_ref()) {
            None => Ok(0),
            Some(Value::String(s)) => Ok(s.len()),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Increments the integer stored at `key` by 1 (a missing key counts as
    /// 0) and returns the new value. Fails with [`Error::NotAnInteger`],
    /// [`Error::Overflow`] or [`Error::WrongType`], leaving the value
    /// unchanged. Redis: `INCR`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.incr("n").unwrap(), 1);
    /// assert_eq!(store.get("n").unwrap(), Some(&b"1"[..]));
    /// ```
    pub fn incr(&mut self, key: impl AsRef<[u8]>) -> Result<i64, Error> {
        self.incrby(key, 1)
    }

    /// Decrements the integer stored at `key` by 1 (a missing key counts as
    /// 0) and returns the new value. Errors as for [`Store::incr`].
    /// Redis: `DECR`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.decr("n").unwrap(), -1);
    /// ```
    pub fn decr(&mut self, key: impl AsRef<[u8]>) -> Result<i64, Error> {
        self.incrby(key, -1)
    }

    /// Adds `by` (which may be negative) to the integer stored at `key` (a
    /// missing key counts as 0) and returns the new value. Errors as for
    /// [`Store::incr`]. Redis: `INCRBY`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.set("n", "10");
    /// assert_eq!(store.incrby("n", -4).unwrap(), 6);
    /// assert_eq!(store.incrby("n", i64::MAX), Err(squall::Error::Overflow));
    /// ```
    pub fn incrby(&mut self, key: impl AsRef<[u8]>, by: i64) -> Result<i64, Error> {
        let current = match self.map.get(key.as_ref()) {
            None => 0,
            Some(Value::String(s)) => parse_i64(s)?,
            Some(_) => return Err(Error::WrongType),
        };
        let new = current.checked_add(by).ok_or(Error::Overflow)?;
        self.map.insert(
            key.as_ref().to_vec(),
            Value::String(new.to_string().into_bytes()),
        );
        Ok(new)
    }
}
