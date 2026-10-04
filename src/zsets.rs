//! Sorted set (zset) commands.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use crate::{Error, Store, value::Value};

/// A non-NaN score with a total order in which `-0.0 == 0.0` (f64's own
/// `partial_cmp` already behaves so). The raw value is kept for `zscore`.
#[derive(Debug, Clone, Copy)]
struct Score(f64);

impl PartialEq for Score {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Score {}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        // NaN is never stored, so partial_cmp always succeeds.
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

/// Internal sorted set: ordered by (score, member bytes), with a member to
/// score map for lookup and update. Never exposed publicly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ZSet {
    ordered: BTreeSet<(Score, Vec<u8>)>,
    scores: HashMap<Vec<u8>, Score>,
}

impl ZSet {
    /// Sets the score of `member`; returns true if it was newly added.
    fn insert(&mut self, score: f64, member: &[u8]) -> bool {
        let old = self.scores.insert(member.to_vec(), Score(score));
        if let Some(old) = old {
            self.ordered.remove(&(old, member.to_vec()));
        }
        self.ordered.insert((Score(score), member.to_vec()));
        old.is_none()
    }

    fn remove(&mut self, member: &[u8]) -> bool {
        match self.scores.remove(member) {
            Some(old) => {
                self.ordered.remove(&(old, member.to_vec()));
                true
            }
            None => false,
        }
    }
}

impl Store {
    /// Adds `(score, member)` pairs to the zset at `key`, creating it if
    /// missing, and returns how many members were newly added. An existing
    /// member's score is updated and not counted; for duplicates within the
    /// call the last score wins. Fails with [`Error::NotAFloat`] if any score
    /// is NaN (nothing is changed), checked before the key's type as Redis
    /// does, and with [`Error::WrongType`] if the key holds a non-zset.
    /// Infinite scores are allowed. Redis: `ZADD` (without
    /// flags).
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.zadd("k", [(1.0, "a"), (2.0, "b")]).unwrap(), 2);
    /// assert_eq!(store.zadd("k", [(3.0, "a"), (4.0, "c")]).unwrap(), 1);
    /// assert_eq!(store.zscore("k", "a").unwrap(), Some(3.0));
    /// assert!(store.zadd("k", [(f64::NAN, "x")]).is_err());
    /// ```
    pub fn zadd<M: AsRef<[u8]>>(
        &mut self,
        key: impl AsRef<[u8]>,
        members: impl IntoIterator<Item = (f64, M)>,
    ) -> Result<usize, Error> {
        let key = key.as_ref();
        // Scores are validated before the key is looked up, as Redis does.
        let members: Vec<(f64, M)> = members.into_iter().collect();
        if members.iter().any(|(score, _)| score.is_nan()) {
            return Err(Error::NotAFloat);
        }
        if let Some(v) = self.map.get(key)
            && !matches!(v, Value::ZSet(_))
        {
            return Err(Error::WrongType);
        }
        if members.is_empty() {
            return Ok(0);
        }
        let Value::ZSet(zset) = self
            .map
            .entry(key.to_vec())
            .or_insert_with(|| Value::ZSet(ZSet::default()))
        else {
            unreachable!("type checked above");
        };
        Ok(members
            .iter()
            .filter(|(score, m)| zset.insert(*score, m.as_ref()))
            .count())
    }

    /// Returns the score of `member` in the zset at `key`; `None` for a
    /// missing key or member. Fails with [`Error::WrongType`] if the key
    /// holds a non-zset. Redis: `ZSCORE`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.zadd("k", [(1.5, "a")]).unwrap();
    /// assert_eq!(store.zscore("k", "a").unwrap(), Some(1.5));
    /// assert_eq!(store.zscore("k", "b").unwrap(), None);
    /// ```
    pub fn zscore(
        &self,
        key: impl AsRef<[u8]>,
        member: impl AsRef<[u8]>,
    ) -> Result<Option<f64>, Error> {
        match self.map.get(key.as_ref()) {
            None => Ok(None),
            Some(Value::ZSet(z)) => Ok(z.scores.get(member.as_ref()).map(|s| s.0)),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Returns the number of members in the zset at `key`; 0 for a missing
    /// key. Fails with [`Error::WrongType`] if the key holds a non-zset.
    /// Redis: `ZCARD`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// assert_eq!(store.zcard("k").unwrap(), 0);
    /// store.zadd("k", [(1.0, "a"), (2.0, "b")]).unwrap();
    /// assert_eq!(store.zcard("k").unwrap(), 2);
    /// ```
    pub fn zcard(&self, key: impl AsRef<[u8]>) -> Result<usize, Error> {
        match self.map.get(key.as_ref()) {
            None => Ok(0),
            Some(Value::ZSet(z)) => Ok(z.scores.len()),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Removes `members` from the zset at `key` and returns how many were
    /// actually removed. Removing the last member deletes the key. Fails with
    /// [`Error::WrongType`] if the key holds a non-zset. Redis: `ZREM`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.zadd("k", [(1.0, "a"), (2.0, "b")]).unwrap();
    /// assert_eq!(store.zrem("k", ["a", "zzz"]).unwrap(), 1);
    /// assert_eq!(store.zrem("k", ["b"]).unwrap(), 1);
    /// assert!(!store.exists("k"));
    /// ```
    pub fn zrem<M: AsRef<[u8]>>(
        &mut self,
        key: impl AsRef<[u8]>,
        members: impl IntoIterator<Item = M>,
    ) -> Result<usize, Error> {
        let key = key.as_ref();
        let (removed, now_empty) = match self.map.get_mut(key) {
            None => return Ok(0),
            Some(Value::ZSet(z)) => {
                let removed = members
                    .into_iter()
                    .filter(|m| z.remove(m.as_ref()))
                    .count();
                (removed, z.scores.is_empty())
            }
            Some(_) => return Err(Error::WrongType),
        };
        if now_empty {
            self.map.remove(key);
        }
        Ok(removed)
    }
}
