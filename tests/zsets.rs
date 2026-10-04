use squall::{Error, Store};

#[test]
fn zadd_creates_key_and_counts_new_members() {
    let mut s = Store::new();
    assert_eq!(s.zadd("k", [(1.0, "a"), (2.0, "b")]).unwrap(), 2);
    assert!(s.exists("k"));
    assert_eq!(s.zcard("k").unwrap(), 2);
    assert_eq!(s.zscore("k", "a").unwrap(), Some(1.0));
    assert_eq!(s.zscore("k", "b").unwrap(), Some(2.0));
}

#[test]
fn zadd_update_is_not_counted() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a")]).unwrap();
    assert_eq!(s.zadd("k", [(5.0, "a"), (2.0, "b")]).unwrap(), 1);
    assert_eq!(s.zscore("k", "a").unwrap(), Some(5.0));
    assert_eq!(s.zcard("k").unwrap(), 2);
}

#[test]
fn zadd_duplicates_in_call_last_wins() {
    let mut s = Store::new();
    assert_eq!(s.zadd("k", [(1.0, "a"), (9.0, "a")]).unwrap(), 1);
    assert_eq!(s.zscore("k", "a").unwrap(), Some(9.0));
    assert_eq!(s.zcard("k").unwrap(), 1);
}

#[test]
fn zadd_empty_batch_on_missing_key_creates_nothing() {
    let mut s = Store::new();
    assert_eq!(s.zadd("k", Vec::<(f64, &str)>::new()).unwrap(), 0);
    assert!(!s.exists("k"));
}

#[test]
fn zadd_nan_rejected_and_changes_nothing() {
    let mut s = Store::new();
    assert_eq!(
        s.zadd("k", [(1.0, "a"), (f64::NAN, "b")]),
        Err(Error::NotAFloat)
    );
    assert!(!s.exists("k"));
    s.zadd("k", [(1.0, "a")]).unwrap();
    assert_eq!(
        s.zadd("k", [(7.0, "a"), (2.0, "c"), (f64::NAN, "d")]),
        Err(Error::NotAFloat)
    );
    assert_eq!(s.zscore("k", "a").unwrap(), Some(1.0));
    assert_eq!(s.zcard("k").unwrap(), 1);
}

#[test]
fn zadd_accepts_infinities() {
    let mut s = Store::new();
    assert_eq!(
        s.zadd("k", [(f64::INFINITY, "hi"), (f64::NEG_INFINITY, "lo")])
            .unwrap(),
        2
    );
    assert_eq!(s.zscore("k", "hi").unwrap(), Some(f64::INFINITY));
    assert_eq!(s.zscore("k", "lo").unwrap(), Some(f64::NEG_INFINITY));
}

#[test]
fn negative_zero_is_same_score_and_stored_value_is_kept() {
    let mut s = Store::new();
    s.zadd("k", [(-0.0, "a")]).unwrap();
    let got = s.zscore("k", "a").unwrap().unwrap();
    assert_eq!(got, 0.0);
    assert!(got.is_sign_negative());
    // Re-adding with 0.0 is an update, not a new member.
    assert_eq!(s.zadd("k", [(0.0, "a")]).unwrap(), 0);
    assert_eq!(s.zcard("k").unwrap(), 1);
}

#[test]
fn members_with_equal_scores_are_distinct() {
    let mut s = Store::new();
    assert_eq!(s.zadd("k", [(1.0, "b"), (1.0, "a"), (-0.0, "c"), (0.0, "d")]).unwrap(), 4);
    assert_eq!(s.zcard("k").unwrap(), 4);
}

#[test]
fn zscore_missing_key_or_member_is_none() {
    let mut s = Store::new();
    assert_eq!(s.zscore("k", "a").unwrap(), None);
    s.zadd("k", [(1.0, "a")]).unwrap();
    assert_eq!(s.zscore("k", "zzz").unwrap(), None);
}

#[test]
fn zcard_missing_key_is_zero() {
    assert_eq!(Store::new().zcard("k").unwrap(), 0);
}

#[test]
fn zrem_counts_removed_and_deletes_key_when_empty() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a"), (2.0, "b")]).unwrap();
    assert_eq!(s.zrem("k", ["a", "zzz"]).unwrap(), 1);
    assert_eq!(s.zscore("k", "a").unwrap(), None);
    assert!(s.exists("k"));
    assert_eq!(s.zrem("k", ["b"]).unwrap(), 1);
    assert!(!s.exists("k"));
}

#[test]
fn zrem_missing_key_is_zero() {
    let mut s = Store::new();
    assert_eq!(s.zrem("k", ["a"]).unwrap(), 0);
    assert!(!s.exists("k"));
}

#[test]
fn zrem_duplicate_members_counted_once() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a"), (2.0, "b")]).unwrap();
    assert_eq!(s.zrem("k", ["a", "a"]).unwrap(), 1);
}

fn full_store() -> Store {
    let mut s = Store::new();
    s.set("str", "v");
    s.rpush("list", ["x", "y"]).unwrap();
    s.hset("hash", "f", "1").unwrap();
    s.sadd("set", ["m"]).unwrap();
    s
}

fn assert_original_data(s: &Store) {
    let mut keys = s.keys();
    keys.sort();
    assert_eq!(
        keys,
        vec![b"hash".to_vec(), b"list".to_vec(), b"set".to_vec(), b"str".to_vec()]
    );
    assert_eq!(s.get("str").unwrap(), Some(&b"v"[..]));
    assert_eq!(s.lrange("list", 0, -1).unwrap(), vec![&b"x"[..], &b"y"[..]]);
    assert_eq!(s.hgetall("hash").unwrap(), vec![(&b"f"[..], &b"1"[..])]);
    assert_eq!(s.smembers("set").unwrap(), vec![&b"m"[..]]);
    assert_eq!(s.llen("list").unwrap(), 2);
    assert_eq!(s.hlen("hash").unwrap(), 1);
    assert_eq!(s.scard("set").unwrap(), 1);
}

#[test]
fn wrong_type_on_every_command_changes_nothing() {
    for key in ["str", "list", "hash", "set"] {
        let mut s = full_store();
        assert_eq!(s.zadd(key, [(1.0, "a")]), Err(Error::WrongType));
        assert_eq!(s.zscore(key, "a"), Err(Error::WrongType));
        assert_eq!(s.zcard(key), Err(Error::WrongType));
        assert_eq!(s.zrem(key, ["a", "x", "m", "f"]), Err(Error::WrongType));
        assert_original_data(&s);
    }
}

#[test]
fn nan_on_wrong_type_key_is_not_a_float_and_changes_nothing() {
    for key in ["str", "list", "hash", "set"] {
        let mut s = full_store();
        assert_eq!(
            s.zadd(key, [(1.0, "a"), (f64::NAN, "b")]),
            Err(Error::NotAFloat)
        );
        assert_original_data(&s);
    }
}

#[test]
fn zrem_duplicates_and_misses_leave_other_members_intact() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a"), (2.0, "b"), (3.0, "c")]).unwrap();
    assert_eq!(s.zrem("k", ["a", "a", "nope"]).unwrap(), 1);
    assert_eq!(s.zcard("k").unwrap(), 2);
    assert_eq!(s.zscore("k", "b").unwrap(), Some(2.0));
    assert_eq!(s.zscore("k", "c").unwrap(), Some(3.0));
    assert_eq!(s.zscore("k", "a").unwrap(), None);
}

#[test]
fn zrem_with_no_members_on_existing_zset_changes_nothing() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a")]).unwrap();
    assert_eq!(s.zrem("k", Vec::<&str>::new()).unwrap(), 0);
    assert_eq!(s.zcard("k").unwrap(), 1);
    assert_eq!(s.zscore("k", "a").unwrap(), Some(1.0));
}

#[test]
fn zadd_wrong_type_with_empty_batch_still_fails() {
    let mut s = full_store();
    assert_eq!(
        s.zadd("str", Vec::<(f64, &str)>::new()),
        Err(Error::WrongType)
    );
}

#[test]
fn set_overwrites_zset() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a")]).unwrap();
    s.set("k", "v");
    assert_eq!(s.zcard("k"), Err(Error::WrongType));
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
}

#[test]
fn zset_key_is_wrong_type_for_other_commands() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a")]).unwrap();
    assert_eq!(s.sadd("k", ["a"]), Err(Error::WrongType));
    assert_eq!(s.get("k"), Err(Error::WrongType));
}

#[test]
fn nan_error_displays() {
    assert!(!Error::NotAFloat.to_string().is_empty());
}
