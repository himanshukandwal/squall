use squall::{Error, Store};

#[test]
fn append_creates_missing_key_and_returns_length() {
    let mut s = Store::new();
    assert_eq!(s.append("k", "abc").unwrap(), 3);
    assert_eq!(s.get("k").unwrap(), Some(&b"abc"[..]));
}

#[test]
fn append_to_existing_returns_combined_length() {
    let mut s = Store::new();
    s.set("k", "abc");
    assert_eq!(s.append("k", "de").unwrap(), 5);
    assert_eq!(s.get("k").unwrap(), Some(&b"abcde"[..]));
}

#[test]
fn strlen_missing_is_zero_and_counts_bytes() {
    let mut s = Store::new();
    assert_eq!(s.strlen("k").unwrap(), 0);
    s.set("k", vec![0u8, 255, 1]);
    assert_eq!(s.strlen("k").unwrap(), 3);
}

#[test]
fn incr_decr_incrby_start_from_zero() {
    let mut s = Store::new();
    assert_eq!(s.incr("a").unwrap(), 1);
    assert_eq!(s.get("a").unwrap(), Some(&b"1"[..]));
    assert_eq!(s.decr("b").unwrap(), -1);
    assert_eq!(s.get("b").unwrap(), Some(&b"-1"[..]));
    assert_eq!(s.incrby("c", 10).unwrap(), 10);
    assert_eq!(s.get("c").unwrap(), Some(&b"10"[..]));
}

#[test]
fn incr_on_numeric_string() {
    let mut s = Store::new();
    s.set("k", "5");
    assert_eq!(s.incr("k").unwrap(), 6);
    assert_eq!(s.decr("k").unwrap(), 5);
}

#[test]
fn counters_accept_stored_bare_zero() {
    let mut s = Store::new();
    s.set("k", "0");
    assert_eq!(s.incr("k"), Ok(1));
    s.set("k", "0");
    assert_eq!(s.decr("k"), Ok(-1));
    s.set("k", "0");
    assert_eq!(s.incrby("k", 5), Ok(5));
    s.set("k", "0");
    assert_eq!(s.incrby("k", -5), Ok(-5));

    assert_eq!(s.incr("z"), Ok(1));
    assert_eq!(s.decr("z"), Ok(0));
    assert_eq!(s.incr("z"), Ok(1));
}

#[test]
fn negative_incrby_subtracts() {
    let mut s = Store::new();
    s.set("k", "10");
    assert_eq!(s.incrby("k", -4).unwrap(), 6);
    assert_eq!(s.get("k").unwrap(), Some(&b"6"[..]));
}

#[test]
fn not_an_integer_leaves_value_unchanged() {
    let mut s = Store::new();
    for bad in ["abc", "", "1.5", " 1", "+1", "99999999999999999999", "007", "00", "-0", "-007"] {
        s.set("k", bad);
        assert_eq!(s.incr("k"), Err(Error::NotAnInteger), "{bad:?}");
        assert_eq!(s.get("k").unwrap(), Some(bad.as_bytes()));
    }
    s.set("k", vec![0xffu8]);
    assert_eq!(s.incr("k"), Err(Error::NotAnInteger));
}

#[test]
fn overflow_at_both_boundaries_leaves_value_unchanged() {
    let mut s = Store::new();
    s.set("k", i64::MAX.to_string());
    assert_eq!(s.incr("k"), Err(Error::Overflow));
    assert_eq!(s.incrby("k", 1), Err(Error::Overflow));
    assert_eq!(s.get("k").unwrap(), Some(i64::MAX.to_string().as_bytes()));
    s.set("k", i64::MIN.to_string());
    assert_eq!(s.decr("k"), Err(Error::Overflow));
    assert_eq!(s.incrby("k", -1), Err(Error::Overflow));
    assert_eq!(s.get("k").unwrap(), Some(i64::MIN.to_string().as_bytes()));
    assert_eq!(s.incrby("k", i64::MAX).unwrap(), -1);
}

#[test]
fn incrby_of_i64_min_on_missing_key_succeeds() {
    let mut s = Store::new();
    assert_eq!(s.incrby("k", i64::MIN).unwrap(), i64::MIN);
    assert_eq!(s.get("k").unwrap(), Some(i64::MIN.to_string().as_bytes()));
}

#[test]
fn wrong_type_on_every_string_command() {
    let mut s = Store::new();
    s.lpush("l", ["a", "b"]).unwrap();
    s.hset("h", "f", "v").unwrap();
    s.sadd("s", ["m"]).unwrap();
    let len = s.len();
    let mut keys = s.keys();
    keys.sort();

    for k in ["l", "h", "s"] {
        assert_eq!(s.append(k, "x"), Err(Error::WrongType), "append {k}");
        assert_eq!(s.strlen(k), Err(Error::WrongType), "strlen {k}");
        assert_eq!(s.incr(k), Err(Error::WrongType), "incr {k}");
        assert_eq!(s.decr(k), Err(Error::WrongType), "decr {k}");
        assert_eq!(s.incrby(k, 5), Err(Error::WrongType), "incrby {k}");
    }

    assert_eq!(s.lrange("l", 0, -1).unwrap(), vec![&b"b"[..], &b"a"[..]]);
    assert_eq!(s.hgetall("h").unwrap(), vec![(&b"f"[..], &b"v"[..])]);
    assert_eq!(s.smembers("s").unwrap(), vec![&b"m"[..]]);
    assert_eq!(s.len(), len);
    let mut after = s.keys();
    after.sort();
    assert_eq!(after, keys);
}
