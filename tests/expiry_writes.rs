use squall::{Error, ManualClock, Store, Ttl};
use std::time::Duration;

const MS: Duration = Duration::from_millis(1);
const SEC: Duration = Duration::from_secs(1);

fn store() -> (Store, ManualClock) {
    let clock = ManualClock::new();
    (Store::with_clock(clock.clone()), clock)
}

#[test]
fn set_ex_writes_the_value_with_its_ttl() {
    let (mut s, clock) = store();
    s.set_ex("k", "v", 10 * SEC);
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
    assert_eq!(s.ttl("k"), Ttl::Remaining(10 * SEC));
    clock.advance(10 * SEC - MS);
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
    clock.advance(MS);
    assert_eq!(s.get("k").unwrap(), None);
    assert_eq!(s.ttl("k"), Ttl::Missing);
}

#[test]
fn set_ex_replaces_an_existing_ttl() {
    let (mut s, clock) = store();
    s.set_ex("k", "a", 10 * SEC);
    clock.advance(4 * SEC);
    s.set_ex("k", "b", 3 * SEC);
    assert_eq!(s.get("k").unwrap(), Some(&b"b"[..]));
    assert_eq!(s.ttl("k"), Ttl::Remaining(3 * SEC));
}

#[test]
fn set_ex_overwrites_a_key_of_any_type() {
    let (mut s, _clock) = store();
    s.rpush("l", ["a"]).unwrap();
    s.hset("h", "f", "v").unwrap();
    s.sadd("s", ["m"]).unwrap();
    s.zadd("z", [(1.0, "m")]).unwrap();
    for key in ["l", "h", "s", "z"] {
        s.set_ex(key, "v", 5 * SEC);
        assert_eq!(s.get(key).unwrap(), Some(&b"v"[..]));
        assert_eq!(s.ttl(key), Ttl::Remaining(5 * SEC));
    }
}

#[test]
fn set_ex_with_zero_duration_leaves_the_key_absent() {
    let (mut s, _clock) = store();
    s.set_ex("new", "v", Duration::ZERO);
    assert!(!s.exists("new"));
    assert_eq!(s.ttl("new"), Ttl::Missing);

    s.set("old", "v");
    s.set_ex("old", "w", Duration::ZERO);
    assert!(!s.exists("old"));
    assert_eq!(s.get("old").unwrap(), None);

    s.rpush("l", ["a"]).unwrap();
    s.expire("l", 10 * SEC);
    s.set_ex("l", "w", Duration::ZERO);
    assert!(!s.exists("l"));
    assert_eq!(s.ttl("l"), Ttl::Missing);
    assert_eq!(s.len(), 0);
    assert_eq!(s.purge_expired(), 0);
}

#[test]
fn set_ex_with_overflowing_duration_stores_with_no_ttl() {
    let (mut s, _clock) = store();
    s.set_ex("k", "v", 10 * SEC);
    s.set_ex("k", "w", Duration::MAX);
    assert_eq!(s.get("k").unwrap(), Some(&b"w"[..]));
    assert_eq!(s.ttl("k"), Ttl::Persistent);
}

#[test]
fn set_ex_on_expired_key_of_another_type_succeeds() {
    let (mut s, clock) = store();
    s.rpush("k", ["a"]).unwrap();
    s.expire("k", SEC);
    clock.advance(SEC);
    s.set_ex("k", "v", 2 * SEC);
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
    assert_eq!(s.ttl("k"), Ttl::Remaining(2 * SEC));
}

#[test]
fn set_clears_the_ttl() {
    let (mut s, clock) = store();
    s.set_ex("k", "a", 10 * SEC);
    s.set("k", "b");
    assert_eq!(s.ttl("k"), Ttl::Persistent);
    clock.advance(20 * SEC);
    assert_eq!(s.get("k").unwrap(), Some(&b"b"[..]));
}

/// Gives `key` a 10s TTL, runs `write` 4s later and checks that the TTL is
/// still running from the original deadline.
fn assert_keeps_ttl(s: &mut Store, clock: &ManualClock, key: &str, write: impl FnOnce(&mut Store)) {
    assert!(s.expire(key, 10 * SEC), "{key} must exist");
    clock.advance(4 * SEC);
    write(s);
    assert_eq!(s.ttl(key), Ttl::Remaining(6 * SEC), "{key}");
}

#[test]
fn string_in_place_writes_keep_the_ttl() {
    let (mut s, clock) = store();
    s.set("a", "x");
    assert_keeps_ttl(&mut s, &clock, "a", |s| {
        assert_eq!(s.append("a", "y").unwrap(), 2);
    });
    s.set("n", "5");
    assert_keeps_ttl(&mut s, &clock, "n", |s| {
        assert_eq!(s.incr("n").unwrap(), 6);
    });
    assert_keeps_ttl(&mut s, &clock, "n", |s| {
        assert_eq!(s.decr("n").unwrap(), 5);
    });
    assert_keeps_ttl(&mut s, &clock, "n", |s| {
        assert_eq!(s.incrby("n", 10).unwrap(), 15);
    });
}

#[test]
fn list_push_and_pop_keep_the_ttl() {
    let (mut s, clock) = store();
    s.rpush("l", ["a", "b"]).unwrap();
    assert_keeps_ttl(&mut s, &clock, "l", |s| {
        assert_eq!(s.lpush("l", ["z"]).unwrap(), 3);
    });
    assert_keeps_ttl(&mut s, &clock, "l", |s| {
        assert_eq!(s.rpush("l", ["c"]).unwrap(), 4);
    });
    assert_keeps_ttl(&mut s, &clock, "l", |s| {
        assert_eq!(s.lpop("l").unwrap(), Some(b"z".to_vec()));
    });
    assert_keeps_ttl(&mut s, &clock, "l", |s| {
        assert_eq!(s.rpop("l").unwrap(), Some(b"c".to_vec()));
    });
}

#[test]
fn hash_set_and_zset_writes_keep_the_ttl() {
    let (mut s, clock) = store();
    s.hset("h", "f", "1").unwrap();
    assert_keeps_ttl(&mut s, &clock, "h", |s| {
        s.hset("h", "g", "2").unwrap();
    });
    assert_keeps_ttl(&mut s, &clock, "h", |s| {
        assert_eq!(s.hdel("h", ["f"]).unwrap(), 1);
    });

    s.sadd("s", ["a"]).unwrap();
    assert_keeps_ttl(&mut s, &clock, "s", |s| {
        assert_eq!(s.sadd("s", ["b"]).unwrap(), 1);
    });
    assert_keeps_ttl(&mut s, &clock, "s", |s| {
        assert_eq!(s.srem("s", ["a"]).unwrap(), 1);
    });

    s.zadd("z", [(1.0, "a")]).unwrap();
    assert_keeps_ttl(&mut s, &clock, "z", |s| {
        assert_eq!(s.zadd("z", [(2.0, "b")]).unwrap(), 1);
    });
    assert_keeps_ttl(&mut s, &clock, "z", |s| {
        assert_eq!(s.zrem("z", ["a"]).unwrap(), 1);
    });
}

#[test]
fn incr_then_expire_rate_limit_pattern() {
    let (mut s, clock) = store();
    assert_eq!(s.incr("rl").unwrap(), 1);
    assert!(s.expire("rl", 60 * SEC));
    clock.advance(20 * SEC);
    assert_eq!(s.incr("rl").unwrap(), 2);
    clock.advance(20 * SEC);
    assert_eq!(s.incr("rl").unwrap(), 3);
    assert_eq!(s.ttl("rl"), Ttl::Remaining(20 * SEC));
    clock.advance(20 * SEC);
    assert_eq!(s.ttl("rl"), Ttl::Missing);
    assert_eq!(s.incr("rl").unwrap(), 1);
    assert_eq!(s.ttl("rl"), Ttl::Persistent);
}

#[test]
fn creating_write_on_expired_key_of_another_type_starts_fresh() {
    let (mut s, clock) = store();
    s.set("k", "v");
    s.expire("k", SEC);
    clock.advance(SEC);
    assert_eq!(s.lpush("k", ["a"]).unwrap(), 1);
    assert_eq!(s.lrange("k", 0, -1).unwrap(), vec![&b"a"[..]]);
    assert_eq!(s.ttl("k"), Ttl::Persistent);

    s.expire("k", SEC);
    clock.advance(SEC);
    assert_eq!(s.sadd("k", ["m"]).unwrap(), 1);
    assert_eq!(s.smembers("k").unwrap(), vec![&b"m"[..]]);
    assert_eq!(s.ttl("k"), Ttl::Persistent);

    s.expire("k", SEC);
    clock.advance(SEC);
    assert_eq!(s.append("k", "xy").unwrap(), 2);
    assert_eq!(s.ttl("k"), Ttl::Persistent);

    s.expire("k", SEC);
    clock.advance(SEC);
    assert_eq!(s.hset("k", "f", "v"), Ok(true));
    assert_eq!(s.hget("k", "f").unwrap(), Some(&b"v"[..]));
    assert_eq!(s.ttl("k"), Ttl::Persistent);

    s.expire("k", SEC);
    clock.advance(SEC);
    assert_eq!(s.zadd("k", [(1.0, "m")]), Ok(1));
    assert_eq!(s.zscore("k", "m").unwrap(), Some(1.0));
    assert_eq!(s.ttl("k"), Ttl::Persistent);
}

#[test]
fn removing_write_on_expired_key_finds_nothing() {
    let (mut s, clock) = store();
    // Each write runs right after its own Key expires, so no earlier
    // command has drained it yet.
    let expire_now = |s: &mut Store, key: &str| {
        s.expire(key, SEC);
        clock.advance(SEC);
    };
    s.rpush("l", ["a", "b", "c"]).unwrap();
    expire_now(&mut s, "l");
    assert_eq!(s.lpop("l"), Ok(None));
    s.rpush("l", ["a", "b", "c"]).unwrap();
    expire_now(&mut s, "l");
    assert_eq!(s.rpop("l"), Ok(None));
    s.hset("h", "f", "v").unwrap();
    expire_now(&mut s, "h");
    assert_eq!(s.hdel("h", ["f"]), Ok(0));
    s.sadd("s", ["m"]).unwrap();
    expire_now(&mut s, "s");
    assert_eq!(s.srem("s", ["m"]), Ok(0));
    s.zadd("z", [(1.0, "m")]).unwrap();
    expire_now(&mut s, "z");
    assert_eq!(s.zrem("z", ["m"]), Ok(0));
    for key in ["l", "h", "s", "z"] {
        assert!(!s.exists(key), "{key}");
        assert_eq!(s.ttl(key), Ttl::Missing, "{key}");
    }
    assert_eq!(s.len(), 0);
}

#[test]
fn emptying_a_container_drops_the_key_and_its_ttl() {
    let (mut s, _clock) = store();
    s.rpush("l", ["a"]).unwrap();
    s.hset("h", "f", "v").unwrap();
    s.sadd("s", ["m"]).unwrap();
    s.zadd("z", [(1.0, "m")]).unwrap();
    for key in ["l", "h", "s", "z"] {
        s.expire(key, 10 * SEC);
    }
    s.lpop("l").unwrap();
    s.hdel("h", ["f"]).unwrap();
    s.srem("s", ["m"]).unwrap();
    s.zrem("z", ["m"]).unwrap();
    for key in ["l", "h", "s", "z"] {
        assert!(!s.exists(key), "{key}");
        assert_eq!(s.ttl(key), Ttl::Missing, "{key}");
    }
    assert_eq!(s.len(), 0);

    s.rpush("l", ["b"]).unwrap();
    s.hset("h", "g", "v").unwrap();
    s.sadd("s", ["n"]).unwrap();
    s.zadd("z", [(2.0, "n")]).unwrap();
    for key in ["l", "h", "s", "z"] {
        assert_eq!(s.ttl(key), Ttl::Persistent, "{key}");
    }
}

#[test]
fn failing_commands_leave_values_and_ttls_unchanged() {
    let (mut s, clock) = store();
    s.set("str", "abc");
    s.set("max", i64::MAX.to_string());
    s.rpush("list", ["a"]).unwrap();
    s.zadd("z", [(1.0, "m")]).unwrap();
    s.hset("hash", "f", "v").unwrap();
    s.sadd("set", ["m"]).unwrap();
    s.set("persistent", "p");
    s.set_ex("doomed", "d", SEC);
    let with_ttl = ["str", "max", "list", "z", "hash", "set"];
    for key in with_ttl {
        s.expire(key, 10 * SEC);
    }
    clock.advance(SEC);

    assert_eq!(s.incr("str"), Err(Error::NotAnInteger));
    assert_eq!(s.incrby("max", 1), Err(Error::Overflow));
    assert_eq!(s.append("list", "x"), Err(Error::WrongType));
    assert_eq!(s.sadd("str", ["m"]), Err(Error::WrongType));
    assert_eq!(s.lpush("z", ["x"]), Err(Error::WrongType));
    assert_eq!(s.zadd("z", [(f64::NAN, "n")]), Err(Error::NotAFloat));
    assert_eq!(s.zadd("list", [(1.0, "n")]), Err(Error::WrongType));
    assert_eq!(s.incr("hash"), Err(Error::WrongType));
    assert_eq!(s.sadd("hash", ["x"]), Err(Error::WrongType));
    assert_eq!(s.hset("set", "f", "x"), Err(Error::WrongType));
    assert_eq!(s.rpush("set", ["x"]), Err(Error::WrongType));

    assert_eq!(s.get("str").unwrap(), Some(&b"abc"[..]));
    assert_eq!(s.get("max").unwrap(), Some(i64::MAX.to_string().as_bytes()));
    assert_eq!(s.lrange("list", 0, -1).unwrap(), vec![&b"a"[..]]);
    assert_eq!(s.zscore("z", "m").unwrap(), Some(1.0));
    assert_eq!(s.zscore("z", "n").unwrap(), None);
    assert_eq!(s.hget("hash", "f").unwrap(), Some(&b"v"[..]));
    assert_eq!(s.smembers("set").unwrap(), vec![&b"m"[..]]);
    assert_eq!(s.get("persistent").unwrap(), Some(&b"p"[..]));
    for key in with_ttl {
        assert_eq!(s.ttl(key), Ttl::Remaining(9 * SEC), "{key}");
    }
    assert_eq!(s.ttl("persistent"), Ttl::Persistent);
    assert_eq!(s.ttl("doomed"), Ttl::Missing);
    assert!(!s.exists("doomed"));
    assert_eq!(s.len(), 7);
}
