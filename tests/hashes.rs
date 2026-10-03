use squall::{Error, Store};
use std::collections::HashSet;

fn pair_set(s: &Store, k: &str) -> HashSet<(Vec<u8>, Vec<u8>)> {
    s.hgetall(k)
        .unwrap()
        .into_iter()
        .map(|(f, v)| (f.to_vec(), v.to_vec()))
        .collect()
}

#[test]
fn hset_reports_new_then_overwrite() {
    let mut s = Store::new();
    assert!(s.hset("h", "f", "1").unwrap());
    assert!(!s.hset("h", "f", "2").unwrap());
    assert_eq!(s.hget("h", "f").unwrap(), Some(&b"2"[..]));
    assert_eq!(s.hlen("h").unwrap(), 1);
}

#[test]
fn hget_hexists_missing() {
    let mut s = Store::new();
    assert_eq!(s.hget("h", "f").unwrap(), None);
    assert!(!s.hexists("h", "f").unwrap());
    s.hset("h", "f", "1").unwrap();
    assert_eq!(s.hget("h", "x").unwrap(), None);
    assert!(!s.hexists("h", "x").unwrap());
    assert!(s.hexists("h", "f").unwrap());
}

#[test]
fn hdel_counts_actually_removed() {
    let mut s = Store::new();
    s.hset("h", "a", "1").unwrap();
    s.hset("h", "b", "2").unwrap();
    s.hset("h", "c", "3").unwrap();
    assert_eq!(s.hdel("h", ["a", "b", "zzz"]).unwrap(), 2);
    assert_eq!(s.hlen("h").unwrap(), 1);
    assert_eq!(s.hdel("missing", ["a"]).unwrap(), 0);
}

#[test]
fn removing_last_field_deletes_key() {
    let mut s = Store::new();
    s.hset("h", "a", "1").unwrap();
    assert_eq!(s.hdel("h", &["a"]).unwrap(), 1);
    assert!(!s.exists("h"));
    assert!(s.keys().is_empty());
    assert_eq!(s.len(), 0);
}

#[test]
fn hdel_with_no_matching_fields_keeps_hash() {
    let mut s = Store::new();
    s.hset("h", "a", "1").unwrap();
    assert_eq!(s.hdel("h", ["x"]).unwrap(), 0);
    assert!(s.exists("h"));
}

#[test]
fn hgetall_hkeys_as_sets_and_empty_when_missing() {
    let mut s = Store::new();
    assert!(s.hgetall("h").unwrap().is_empty());
    assert!(s.hkeys("h").unwrap().is_empty());
    assert_eq!(s.hlen("h").unwrap(), 0);
    s.hset("h", "a", "1").unwrap();
    s.hset("h", "b", "2").unwrap();
    let want: HashSet<_> = [
        (b"a".to_vec(), b"1".to_vec()),
        (b"b".to_vec(), b"2".to_vec()),
    ]
    .into();
    assert_eq!(pair_set(&s, "h"), want);
    let keys: HashSet<Vec<u8>> = s
        .hkeys("h")
        .unwrap()
        .into_iter()
        .map(|k| k.to_vec())
        .collect();
    assert_eq!(keys, [b"a".to_vec(), b"b".to_vec()].into());
}

#[test]
fn binary_safe_fields_and_values() {
    let mut s = Store::new();
    s.hset(&b"h"[..], vec![0u8, 255], vec![1u8, 0, 2]).unwrap();
    assert_eq!(s.hget(b"h", [0u8, 255]).unwrap(), Some(&[1u8, 0, 2][..]));
}

#[test]
fn wrong_type_on_every_hash_command() {
    let mut s = Store::new();
    s.set("k", "v");
    assert_eq!(s.hset("k", "f", "v"), Err(Error::WrongType));
    assert_eq!(s.hget("k", "f"), Err(Error::WrongType));
    assert_eq!(s.hdel("k", ["f"]), Err(Error::WrongType));
    assert_eq!(s.hexists("k", "f"), Err(Error::WrongType));
    assert_eq!(s.hlen("k"), Err(Error::WrongType));
    assert_eq!(s.hgetall("k"), Err(Error::WrongType));
    assert_eq!(s.hkeys("k"), Err(Error::WrongType));
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
}
