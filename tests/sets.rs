use squall::{Error, Store};
use std::collections::HashSet;

fn members(s: &Store, key: &str) -> HashSet<Vec<u8>> {
    s.smembers(key)
        .unwrap()
        .into_iter()
        .map(|m| m.to_vec())
        .collect()
}

fn set_of(items: &[&[u8]]) -> HashSet<Vec<u8>> {
    items.iter().map(|m| m.to_vec()).collect()
}

#[test]
fn sadd_creates_set_and_counts_new_members() {
    let mut s = Store::new();
    assert_eq!(s.sadd("k", ["a", "b", "c"]).unwrap(), 3);
    assert!(s.exists("k"));
    assert_eq!(members(&s, "k"), set_of(&[b"a", b"b", b"c"]));
}

#[test]
fn sadd_duplicates_within_and_across_calls() {
    let mut s = Store::new();
    assert_eq!(s.sadd("k", ["a", "a", "b"]).unwrap(), 2);
    assert_eq!(s.sadd("k", ["b", "c", "c"]).unwrap(), 1);
    assert_eq!(s.scard("k").unwrap(), 3);
}

#[test]
fn sadd_accepts_vec_and_slice_inputs() {
    let mut s = Store::new();
    assert_eq!(s.sadd("k", vec![b"x".to_vec()]).unwrap(), 1);
    assert_eq!(s.sadd("k", [&b"y"[..]]).unwrap(), 1);
}

#[test]
fn members_are_binary_safe() {
    let mut s = Store::new();
    s.sadd("k", [&b"\x00\xff"[..], &b"\x00"[..]]).unwrap();
    assert!(s.sismember("k", b"\x00\xff").unwrap());
    assert!(!s.sismember("k", b"\xff").unwrap());
    assert_eq!(s.scard("k").unwrap(), 2);
}

#[test]
fn srem_counts_actually_removed() {
    let mut s = Store::new();
    s.sadd("k", ["a", "b", "c"]).unwrap();
    assert_eq!(s.srem("k", ["a", "zzz", "a"]).unwrap(), 1);
    assert_eq!(members(&s, "k"), set_of(&[b"b", b"c"]));
}

#[test]
fn srem_on_missing_key_is_zero() {
    let mut s = Store::new();
    assert_eq!(s.srem("k", ["a"]).unwrap(), 0);
    assert!(!s.exists("k"));
}

#[test]
fn removing_last_member_deletes_key() {
    let mut s = Store::new();
    s.sadd("k", ["a", "b"]).unwrap();
    assert_eq!(s.srem("k", ["a", "b"]).unwrap(), 2);
    assert!(!s.exists("k"));
    assert!(s.keys().is_empty());
    assert_eq!(s.len(), 0);
}

#[test]
fn sadd_with_no_members_does_not_create_key() {
    let mut s = Store::new();
    assert_eq!(s.sadd("k", Vec::<&str>::new()).unwrap(), 0);
    assert!(!s.exists("k"));
}

#[test]
fn missing_key_reads() {
    let s = Store::new();
    assert!(!s.sismember("k", "a").unwrap());
    assert_eq!(s.scard("k").unwrap(), 0);
    assert!(s.smembers("k").unwrap().is_empty());
}

#[test]
fn sismember_and_scard() {
    let mut s = Store::new();
    s.sadd("k", ["a", "b"]).unwrap();
    assert!(s.sismember("k", "a").unwrap());
    assert!(!s.sismember("k", "c").unwrap());
    assert_eq!(s.scard("k").unwrap(), 2);
}

#[test]
fn wrong_type_on_every_set_command() {
    let mut s = Store::new();
    s.set("str", "v");
    assert_eq!(s.sadd("str", ["a"]), Err(Error::WrongType));
    assert_eq!(s.srem("str", ["a"]), Err(Error::WrongType));
    assert_eq!(s.sismember("str", "a"), Err(Error::WrongType));
    assert_eq!(s.scard("str"), Err(Error::WrongType));
    assert_eq!(s.smembers("str"), Err(Error::WrongType));
    assert_eq!(s.get("str").unwrap(), Some(&b"v"[..]));
}

#[test]
fn set_overwrites_a_set() {
    let mut s = Store::new();
    s.sadd("k", ["a"]).unwrap();
    s.set("k", "v");
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
}
