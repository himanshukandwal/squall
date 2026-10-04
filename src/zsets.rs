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

    /// Members with scores in rank order for the inclusive, possibly negative
    /// index window, clamped lrange-style. O(n) skip to `start`.
    fn window(&self, start: i64, stop: i64) -> impl Iterator<Item = &(Score, Vec<u8>)> {
        let len = self.ordered.len() as i128;
        let resolve = |i: i64| {
            let i = i128::from(i);
            if i < 0 { i + len } else { i }
        };
        let start = resolve(start).max(0);
        let stop = resolve(stop).min(len - 1);
        let take = if start > stop {
            0
        } else {
            (stop - start + 1) as usize
        };
        self.ordered.iter().skip(start as usize).take(take)
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
                let removed = members.into_iter().filter(|m| z.remove(m.as_ref())).count();
                (removed, z.scores.is_empty())
            }
            Some(_) => return Err(Error::WrongType),
        };
        if now_empty {
            self.map.remove(key);
        }
        Ok(removed)
    }

    fn zset(&self, key: &[u8]) -> Result<Option<&ZSet>, Error> {
        match self.map.get(key) {
            None => Ok(None),
            Some(Value::ZSet(z)) => Ok(Some(z)),
            Some(_) => Err(Error::WrongType),
        }
    }

    /// Returns the members at ranks `start..=stop` in ascending score order
    /// (ties by member bytes). Indices follow `lrange`: negative counts from
    /// the end, `stop` is inclusive, out-of-range indices are clamped, and
    /// an empty window or missing key gives an empty result. Fails with
    /// [`Error::WrongType`] if the key holds a non-zset. Skipping to `start`
    /// is O(start). Redis: `ZRANGE` (without options).
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.zadd("k", [(3.0, "c"), (1.0, "a"), (2.0, "b")]).unwrap();
    /// assert_eq!(store.zrange("k", 0, 1).unwrap(), vec![&b"a"[..], &b"b"[..]]);
    /// assert_eq!(store.zrange("k", -1, -1).unwrap(), vec![&b"c"[..]]);
    /// assert!(store.zrange("k", 2, 1).unwrap().is_empty());
    /// ```
    pub fn zrange(
        &self,
        key: impl AsRef<[u8]>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<&[u8]>, Error> {
        Ok(self
            .zset(key.as_ref())?
            .map(|z| z.window(start, stop).map(|(_, m)| m.as_slice()).collect())
            .unwrap_or_default())
    }

    /// Like [`Store::zrange`] but returns `(member, score)` pairs.
    /// Redis: `ZRANGE ... WITHSCORES`.
    ///
    /// ```
    /// let mut store = squall::Store::new();
    /// store.zadd("k", [(2.0, "b"), (1.0, "a")]).unwrap();
    /// assert_eq!(
    ///     store.zrange_withscores("k", 0, -1).unwrap(),
    ///     vec![(&b"a"[..], 1.0), (&b"b"[..], 2.0)]
    /// );
    /// ```
    pub fn zrange_withscores(
        &self,
        key: impl AsRef<[u8]>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<(&[u8], f64)>, Error> {
        let Some(z) = self.zset(key.as_ref())? else {
            return Ok(Vec::new());
        };
        Ok(z.window(start, stop)
            .map(|(_, m)| (m.as_slice(), z.scores[m].0))
            .collect())
    }
}
