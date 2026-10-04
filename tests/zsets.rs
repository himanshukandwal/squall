use squall::{Bound, Error, Store};

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
    assert_eq!(
        s.zadd("k", [(1.0, "b"), (1.0, "a"), (-0.0, "c"), (0.0, "d")])
            .unwrap(),
        4
    );
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
        vec![
            b"hash".to_vec(),
            b"list".to_vec(),
            b"set".to_vec(),
            b"str".to_vec()
        ]
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

fn names(v: Vec<&[u8]>) -> Vec<&str> {
    v.into_iter()
        .map(|m| std::str::from_utf8(m).unwrap())
        .collect()
}

#[test]
fn zrange_orders_by_score() {
    let mut s = Store::new();
    s.zadd("k", [(3.0, "c"), (1.0, "a"), (2.0, "b")]).unwrap();
    assert_eq!(names(s.zrange("k", 0, -1).unwrap()), ["a", "b", "c"]);
}

#[test]
fn zrange_ties_broken_by_member_bytes() {
    let mut s = Store::new();
    // Inserted deliberately out of byte order, all with equal scores.
    s.zadd(
        "k",
        [
            (1.0, "pear"),
            (1.0, "apple"),
            (1.0, "zebra"),
            (1.0, "mango"),
        ],
    )
    .unwrap();
    s.zadd("k", [(0.5, "zzz"), (2.0, "aaa")]).unwrap();
    assert_eq!(
        names(s.zrange("k", 0, -1).unwrap()),
        ["zzz", "apple", "mango", "pear", "zebra", "aaa"]
    );
}

#[test]
fn zrange_tie_break_is_bytewise_not_utf8_aware() {
    let mut s = Store::new();
    s.zadd(
        "k",
        [(1.0, &b"\xff"[..]), (1.0, &b"a"[..]), (1.0, &b"B"[..])],
    )
    .unwrap();
    assert_eq!(
        s.zrange("k", 0, -1).unwrap(),
        vec![&b"B"[..], &b"a"[..], &b"\xff"[..]]
    );
}

#[test]
fn zrange_negative_and_positive_zero_tie_then_bytes() {
    let mut s = Store::new();
    s.zadd("k", [(0.0, "b"), (-0.0, "c"), (0.0, "a"), (-0.0, "d")])
        .unwrap();
    s.zadd("k", [(-1.0, "neg"), (1.0, "pos")]).unwrap();
    assert_eq!(
        names(s.zrange("k", 0, -1).unwrap()),
        ["neg", "a", "b", "c", "d", "pos"]
    );
    // zscore still returns the stored value, sign included.
    assert!(s.zscore("k", "c").unwrap().unwrap().is_sign_negative());
    assert!(s.zscore("k", "b").unwrap().unwrap().is_sign_positive());
}

#[test]
fn zrange_infinite_scores_at_both_ends() {
    let mut s = Store::new();
    s.zadd(
        "k",
        [
            (f64::INFINITY, "hi2"),
            (0.0, "mid"),
            (f64::NEG_INFINITY, "lo2"),
            (f64::MAX, "max"),
            (f64::INFINITY, "hi1"),
            (f64::MIN, "min"),
            (f64::NEG_INFINITY, "lo1"),
        ],
    )
    .unwrap();
    assert_eq!(
        names(s.zrange("k", 0, -1).unwrap()),
        ["lo1", "lo2", "min", "mid", "max", "hi1", "hi2"]
    );
}

#[test]
fn zrange_update_moves_member() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a"), (2.0, "b"), (3.0, "c")]).unwrap();
    s.zadd("k", [(10.0, "a")]).unwrap();
    assert_eq!(names(s.zrange("k", 0, -1).unwrap()), ["b", "c", "a"]);
}

#[test]
fn zrange_index_rules_match_lrange() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "a"), (2.0, "b"), (3.0, "c"), (4.0, "d")])
        .unwrap();
    assert_eq!(names(s.zrange("k", 1, 2).unwrap()), ["b", "c"]);
    assert_eq!(names(s.zrange("k", -2, -1).unwrap()), ["c", "d"]);
    assert_eq!(
        names(s.zrange("k", -100, 100).unwrap()),
        ["a", "b", "c", "d"]
    );
    assert_eq!(names(s.zrange("k", 0, 0).unwrap()), ["a"]);
    assert_eq!(names(s.zrange("k", -1, -1).unwrap()), ["d"]);
    assert!(s.zrange("k", 2, 1).unwrap().is_empty());
    assert!(s.zrange("k", 4, 10).unwrap().is_empty());
    assert!(s.zrange("k", -100, -50).unwrap().is_empty());
    assert!(s.zrange("k", -1, -2).unwrap().is_empty());
    assert_eq!(names(s.zrange("k", i64::MIN, i64::MAX).unwrap()).len(), 4);
    assert!(s.zrange("k", i64::MAX, i64::MIN).unwrap().is_empty());
}

#[test]
fn zrange_missing_key_is_empty() {
    let s = Store::new();
    assert!(s.zrange("nope", 0, -1).unwrap().is_empty());
    assert!(s.zrange_withscores("nope", 0, -1).unwrap().is_empty());
}

#[test]
fn zrange_wrong_type() {
    let mut s = Store::new();
    s.rpush("l", ["a"]).unwrap();
    assert_eq!(s.zrange("l", 0, -1), Err(Error::WrongType));
    assert_eq!(s.zrange_withscores("l", 0, -1), Err(Error::WrongType));
}

#[test]
fn zrange_withscores_pairs() {
    let mut s = Store::new();
    s.zadd(
        "k",
        [
            (2.5, "b"),
            (f64::NEG_INFINITY, "a"),
            (2.5, "a2"),
            (9.0, "z"),
        ],
    )
    .unwrap();
    assert_eq!(
        s.zrange_withscores("k", 0, -1).unwrap(),
        vec![
            (&b"a"[..], f64::NEG_INFINITY),
            (&b"a2"[..], 2.5),
            (&b"b"[..], 2.5),
            (&b"z"[..], 9.0),
        ]
    );
    assert_eq!(
        s.zrange_withscores("k", -2, -2).unwrap(),
        vec![(&b"b"[..], 2.5)]
    );
}

fn score_store() -> Store {
    let mut s = Store::new();
    s.zadd(
        "k",
        [
            (f64::NEG_INFINITY, "ninf"),
            (-0.0, "negzero"),
            (0.0, "zero"),
            (1.0, "a"),
            (2.0, "b"),
            (3.0, "c"),
            (f64::INFINITY, "pinf"),
        ],
    )
    .unwrap();
    s
}

const NEG: Bound = Bound::Inclusive(f64::NEG_INFINITY);
const POS: Bound = Bound::Inclusive(f64::INFINITY);
const NO_NAMES: [&str; 0] = [];

#[test]
fn zrangebyscore_inclusive_and_exclusive_bounds() {
    let s = score_store();
    let r = |lo, hi| names(s.zrangebyscore("k", lo, hi).unwrap());
    let (i, e) = (Bound::Inclusive, Bound::Exclusive);
    assert_eq!(r(i(1.0), i(3.0)), ["a", "b", "c"]);
    assert_eq!(r(e(1.0), i(3.0)), ["b", "c"]);
    assert_eq!(r(i(1.0), e(3.0)), ["a", "b"]);
    assert_eq!(r(e(1.0), e(3.0)), ["b"]);
    assert_eq!(r(i(1.5), i(2.5)), ["b"]);
}

#[test]
fn zrangebyscore_infinite_bounds_and_member_scores() {
    let s = score_store();
    let r = |lo, hi| names(s.zrangebyscore("k", lo, hi).unwrap());
    let (i, e) = (Bound::Inclusive, Bound::Exclusive);
    assert_eq!(
        r(NEG, POS),
        ["ninf", "negzero", "zero", "a", "b", "c", "pinf"]
    );
    // An exclusive infinite bound drops members scored exactly at infinity.
    assert_eq!(
        r(e(f64::NEG_INFINITY), e(f64::INFINITY)),
        ["negzero", "zero", "a", "b", "c"]
    );
    assert_eq!(r(i(3.0), POS), ["c", "pinf"]);
    assert_eq!(r(NEG, i(-1.0)), ["ninf"]);
}

#[test]
fn zrangebyscore_signed_zero_bounds() {
    let s = score_store();
    let r = |lo, hi| names(s.zrangebyscore("k", lo, hi).unwrap());
    let (i, e) = (Bound::Inclusive, Bound::Exclusive);
    assert_eq!(r(i(0.0), i(0.0)), ["negzero", "zero"]);
    assert_eq!(r(i(-0.0), i(-0.0)), ["negzero", "zero"]);
    assert_eq!(r(e(-0.0), i(1.0)), ["a"]);
    assert_eq!(r(e(0.0), i(1.0)), ["a"]);
    assert_eq!(r(i(-1.0), e(0.0)), NO_NAMES);
    assert_eq!(r(i(-1.0), e(-0.0)), NO_NAMES);
}

#[test]
fn zrangebyscore_ties_ordered_by_member_bytes() {
    let mut s = Store::new();
    s.zadd("k", [(1.0, "b"), (1.0, "a"), (1.0, "c"), (2.0, "z")])
        .unwrap();
    let one = Bound::Inclusive(1.0);
    assert_eq!(
        names(s.zrangebyscore("k", one, one).unwrap()),
        ["a", "b", "c"]
    );
}

#[test]
fn zrangebyscore_min_above_max_and_empty_intervals() {
    let s = score_store();
    let r = |lo, hi| names(s.zrangebyscore("k", lo, hi).unwrap());
    let (i, e) = (Bound::Inclusive, Bound::Exclusive);
    assert_eq!(r(i(3.0), i(1.0)), NO_NAMES);
    assert_eq!(r(e(2.0), i(2.0)), NO_NAMES);
    assert_eq!(r(i(2.0), e(2.0)), NO_NAMES);
    assert_eq!(r(POS, NEG), NO_NAMES);
}

#[test]
fn zrangebyscore_missing_key_is_empty() {
    let s = Store::new();
    assert!(s.zrangebyscore("nope", NEG, POS).unwrap().is_empty());
    assert!(
        s.zrangebyscore_withscores("nope", NEG, POS)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn zrangebyscore_withscores_pairs() {
    let s = score_store();
    assert_eq!(
        s.zrangebyscore_withscores("k", Bound::Exclusive(0.0), POS)
            .unwrap(),
        vec![
            (&b"a"[..], 1.0),
            (&b"b"[..], 2.0),
            (&b"c"[..], 3.0),
            (&b"pinf"[..], f64::INFINITY),
        ]
    );
}

#[test]
fn zrangebyscore_nan_bound_rejected_before_type_check_and_missing_key() {
    let mut s = full_store();
    s.zadd("z", [(1.0, "a")]).unwrap();
    let nan = Bound::Inclusive(f64::NAN);
    let xnan = Bound::Exclusive(f64::NAN);
    for key in ["z", "missing", "str", "list", "hash", "set"] {
        assert_eq!(s.zrangebyscore(key, nan, POS), Err(Error::NotAFloat));
        assert_eq!(s.zrangebyscore(key, NEG, xnan), Err(Error::NotAFloat));
        assert_eq!(
            s.zrangebyscore_withscores(key, nan, POS),
            Err(Error::NotAFloat)
        );
        assert_eq!(
            s.zrangebyscore_withscores(key, NEG, xnan),
            Err(Error::NotAFloat)
        );
    }
}

#[test]
fn zrangebyscore_wrong_type() {
    let s = full_store();
    for key in ["str", "list", "hash", "set"] {
        assert_eq!(s.zrangebyscore(key, NEG, POS), Err(Error::WrongType));
        assert_eq!(
            s.zrangebyscore_withscores(key, NEG, POS),
            Err(Error::WrongType)
        );
    }
}
