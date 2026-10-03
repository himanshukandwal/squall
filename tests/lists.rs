use squall::{Error, Store};

fn range(s: &Store, key: &str, start: i64, stop: i64) -> Vec<Vec<u8>> {
    s.lrange(key, start, stop)
        .unwrap()
        .into_iter()
        .map(|e| e.to_vec())
        .collect()
}

fn b(items: &[&str]) -> Vec<Vec<u8>> {
    items.iter().map(|s| s.as_bytes().to_vec()).collect()
}

#[test]
fn rpush_preserves_order_and_returns_length() {
    let mut s = Store::new();
    assert_eq!(s.rpush("l", ["a", "b", "c"]).unwrap(), 3);
    assert_eq!(s.rpush("l", ["d"]).unwrap(), 4);
    assert_eq!(range(&s, "l", 0, -1), b(&["a", "b", "c", "d"]));
}

#[test]
fn lpush_with_no_values_does_not_create_key() {
    let mut s = Store::new();
    assert_eq!(s.lpush("x", Vec::<&str>::new()).unwrap(), 0);
    assert!(!s.exists("x"));
    assert!(s.keys().is_empty());
    assert_eq!(s.len(), 0);
}

#[test]
fn rpush_with_no_values_does_not_create_key() {
    let mut s = Store::new();
    assert_eq!(s.rpush("x", Vec::<&str>::new()).unwrap(), 0);
    assert!(!s.exists("x"));
    assert!(s.keys().is_empty());
    assert_eq!(s.len(), 0);
}

#[test]
fn lpush_reverses_argument_order_and_returns_length() {
    let mut s = Store::new();
    assert_eq!(s.lpush("l", ["a", "b", "c"]).unwrap(), 3);
    assert_eq!(range(&s, "l", 0, -1), b(&["c", "b", "a"]));
    assert_eq!(s.lpush("l", vec![b"z".to_vec()]).unwrap(), 4);
    assert_eq!(range(&s, "l", 0, 0), b(&["z"]));
}

#[test]
fn push_creates_missing_list() {
    let mut s = Store::new();
    s.lpush("a", ["x"]).unwrap();
    s.rpush("b", ["x"]).unwrap();
    assert!(s.exists("a") && s.exists("b"));
    assert_eq!(s.llen("a").unwrap(), 1);
}

#[test]
fn pops_return_owned_elements() {
    let mut s = Store::new();
    s.rpush("l", ["a", "b", "c"]).unwrap();
    let head: Option<Vec<u8>> = s.lpop("l").unwrap();
    let tail: Option<Vec<u8>> = s.rpop("l").unwrap();
    assert_eq!(head, Some(b"a".to_vec()));
    assert_eq!(tail, Some(b"c".to_vec()));
    assert_eq!(range(&s, "l", 0, -1), b(&["b"]));
}

#[test]
fn pop_missing_is_none() {
    let mut s = Store::new();
    assert_eq!(s.lpop("nope").unwrap(), None);
    assert_eq!(s.rpop("nope").unwrap(), None);
}

#[test]
fn popping_last_element_deletes_key() {
    let mut s = Store::new();
    s.rpush("l", ["a"]).unwrap();
    assert_eq!(s.lpop("l").unwrap(), Some(b"a".to_vec()));
    assert!(!s.exists("l"));
    assert!(s.keys().is_empty());
    assert_eq!(s.len(), 0);

    s.rpush("l", ["a"]).unwrap();
    assert_eq!(s.rpop("l").unwrap(), Some(b"a".to_vec()));
    assert!(!s.exists("l"));
    assert!(s.keys().is_empty());
}

#[test]
fn llen_counts_and_missing_is_zero() {
    let mut s = Store::new();
    assert_eq!(s.llen("l").unwrap(), 0);
    s.rpush("l", ["a", "b"]).unwrap();
    assert_eq!(s.llen("l").unwrap(), 2);
}

#[test]
fn lindex_huge_index_does_not_truncate() {
    let mut s = Store::new();
    s.rpush("l", ["a", "b", "c"]).unwrap();
    assert_eq!(s.lindex("l", 1i64 << 32).unwrap(), None);
    assert_eq!(s.lindex("l", i64::MAX).unwrap(), None);
}

#[test]
fn lindex_positive_negative_and_out_of_range() {
    let mut s = Store::new();
    s.rpush("l", ["a", "b", "c"]).unwrap();
    assert_eq!(s.lindex("l", 0).unwrap(), Some(&b"a"[..]));
    assert_eq!(s.lindex("l", 2).unwrap(), Some(&b"c"[..]));
    assert_eq!(s.lindex("l", -1).unwrap(), Some(&b"c"[..]));
    assert_eq!(s.lindex("l", -3).unwrap(), Some(&b"a"[..]));
    assert_eq!(s.lindex("l", 3).unwrap(), None);
    assert_eq!(s.lindex("l", -4).unwrap(), None);
    assert_eq!(s.lindex("l", i64::MIN).unwrap(), None);
    assert_eq!(s.lindex("l", i64::MAX).unwrap(), None);
    assert_eq!(s.lindex("nope", 0).unwrap(), None);
}

#[test]
fn lrange_inclusive_negative_and_clamped() {
    let mut s = Store::new();
    s.rpush("l", ["a", "b", "c", "d"]).unwrap();
    assert_eq!(range(&s, "l", 0, 1), b(&["a", "b"]));
    assert_eq!(range(&s, "l", 1, 1), b(&["b"]));
    assert_eq!(range(&s, "l", 0, -1), b(&["a", "b", "c", "d"]));
    assert_eq!(range(&s, "l", -2, -1), b(&["c", "d"]));
    assert_eq!(range(&s, "l", -100, 100), b(&["a", "b", "c", "d"]));
    assert_eq!(range(&s, "l", 2, 100), b(&["c", "d"]));
    assert_eq!(range(&s, "l", i64::MIN, i64::MAX), b(&["a", "b", "c", "d"]));
}

#[test]
fn lrange_empty_windows() {
    let mut s = Store::new();
    s.rpush("l", ["a", "b", "c"]).unwrap();
    assert!(range(&s, "l", 2, 1).is_empty());
    assert!(range(&s, "l", 5, 10).is_empty());
    assert!(range(&s, "l", -10, -5).is_empty());
    assert!(range(&s, "l", -1, -2).is_empty());
    assert!(range(&s, "nope", 0, -1).is_empty());
}

#[test]
fn wrong_type_on_string_key() {
    let mut s = Store::new();
    s.set("k", "v");
    assert_eq!(s.lpush("k", ["a"]), Err(Error::WrongType));
    assert_eq!(s.rpush("k", ["a"]), Err(Error::WrongType));
    assert_eq!(s.lpop("k"), Err(Error::WrongType));
    assert_eq!(s.rpop("k"), Err(Error::WrongType));
    assert_eq!(s.llen("k"), Err(Error::WrongType));
    assert_eq!(s.lindex("k", 0), Err(Error::WrongType));
    assert_eq!(s.lrange("k", 0, -1), Err(Error::WrongType));
    // failed commands leave the string untouched
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
}
