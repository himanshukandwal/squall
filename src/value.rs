use std::collections::{HashMap, HashSet, VecDeque};

/// Internal representation of a stored value. Never exposed publicly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Value {
    String(Vec<u8>),
    List(VecDeque<Vec<u8>>),
    Hash(HashMap<Vec<u8>, Vec<u8>>),
    Set(HashSet<Vec<u8>>),
}
