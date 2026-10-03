use squall::{Error, Store};
use std::collections::HashSet;

fn key_set(s: &Store) -> HashSet<Vec<u8>> {
    s.keys().into_iter().collect()
}

#[test]
fn new_store_is_empty() {
    let s = Store::new();
    assert_eq!(s.len(), 0);
    assert!(s.keys().is_empty());
}

#[test]
fn set_get_roundtrip_text_and_binary() {
    let mut s = Store::new();
    s.set("a", "hello");
    s.set(b"\x00\xff\x01".to_vec(), vec![0u8, 255, 10, 0]);
    assert_eq!(s.get("a").unwrap(), Some(&b"hello"[..]));
    assert_eq!(
        s.get(&b"\x00\xff\x01"[..]).unwrap(),
        Some(&[0u8, 255, 10, 0][..])
    );
}

#[test]
fn accepts_byte_like_inputs() {
    let mut s = Store::new();
    s.set(String::from("k"), String::from("v"));
    s.set(&b"k2"[..], &b"v2"[..]);
    assert_eq!(s.get(b"k").unwrap(), Some(&b"v"[..]));
    assert_eq!(s.get(String::from("k2")).unwrap(), Some(&b"v2"[..]));
}

#[test]
fn get_missing_is_none() {
    assert_eq!(Store::new().get("nope").unwrap(), None);
}

#[test]
fn set_overwrites() {
    let mut s = Store::new();
    s.set("a", "1");
    s.set("a", "2");
    assert_eq!(s.get("a").unwrap(), Some(&b"2"[..]));
    assert_eq!(s.len(), 1);
}

#[test]
fn del_reports_existence() {
    let mut s = Store::new();
    s.set("a", "1");
    assert!(s.del("a"));
    assert!(!s.del("a"));
    assert!(!s.exists("a"));
    assert_eq!(s.len(), 0);
}

#[test]
fn exists_keys_len_agree() {
    let mut s = Store::new();
    s.set("a", "1");
    s.set("b", "2");
    assert!(s.exists("a") && s.exists("b") && !s.exists("c"));
    assert_eq!(s.len(), 2);
    let expected: HashSet<Vec<u8>> = [b"a".to_vec(), b"b".to_vec()].into_iter().collect();
    assert_eq!(key_set(&s), expected);
    s.del("a");
    assert_eq!(s.len(), 1);
    assert_eq!(key_set(&s), [b"b".to_vec()].into_iter().collect());
}

#[test]
fn keys_are_owned_and_survive_mutation() {
    let mut s = Store::new();
    s.set("a", "1");
    let ks = s.keys();
    s.del("a");
    assert_eq!(ks, vec![b"a".to_vec()]);
}

#[test]
fn error_traits() {
    fn assert_err<E: std::error::Error + std::fmt::Display>() {}
    assert_err::<Error>();
    assert!(!Error::WrongType.to_string().is_empty());
    assert!(!Error::NotAnInteger.to_string().is_empty());
    assert!(!Error::Overflow.to_string().is_empty());
}

// ---- Cross-type behaviour ----

const STR: &str = "str";
const LIST: &str = "list";
const HASH: &str = "hash";
const SET: &str = "set";

fn mixed() -> Store {
    let mut s = Store::new();
    s.set(STR, "5");
    s.rpush(LIST, ["a", "b"]).unwrap();
    s.hset(HASH, "f", "v").unwrap();
    s.sadd(SET, ["m"]).unwrap();
    s
}

/// Observable state of the whole store, read through the public API only.
fn snapshot(s: &Store) -> Vec<(Vec<u8>, String)> {
    let mut out: Vec<(Vec<u8>, String)> = s
        .keys()
        .into_iter()
        .map(|k| {
            let repr = if let Ok(Some(v)) = s.get(&k) {
                format!("str:{v:?}")
            } else if s.llen(&k).is_ok() {
                format!("list:{:?}", s.lrange(&k, 0, -1).unwrap())
            } else if s.hlen(&k).is_ok() {
                let mut h = s.hgetall(&k).unwrap();
                h.sort();
                format!("hash:{h:?}")
            } else {
                let mut m = s.smembers(&k).unwrap();
                m.sort();
                format!("set:{m:?}")
            };
            (k, repr)
        })
        .collect();
    out.sort();
    out
}

type Cmd = fn(&mut Store, &str) -> Result<(), Error>;

fn string_cmds() -> Vec<(&'static str, Cmd)> {
    vec![
        ("get", |s, k| s.get(k).map(drop)),
        ("append", |s, k| s.append(k, "x").map(drop)),
        ("strlen", |s, k| s.strlen(k).map(drop)),
        ("incr", |s, k| s.incr(k).map(drop)),
        ("decr", |s, k| s.decr(k).map(drop)),
        ("incrby", |s, k| s.incrby(k, 2).map(drop)),
    ]
}

fn list_cmds() -> Vec<(&'static str, Cmd)> {
    vec![
        ("lpush", |s, k| s.lpush(k, ["x"]).map(drop)),
        ("rpush", |s, k| s.rpush(k, ["x"]).map(drop)),
        ("lpop", |s, k| s.lpop(k).map(drop)),
        ("rpop", |s, k| s.rpop(k).map(drop)),
        ("llen", |s, k| s.llen(k).map(drop)),
        ("lindex", |s, k| s.lindex(k, 0).map(drop)),
        ("lrange", |s, k| s.lrange(k, 0, -1).map(drop)),
    ]
}

fn hash_cmds() -> Vec<(&'static str, Cmd)> {
    vec![
        ("hset", |s, k| s.hset(k, "f", "x").map(drop)),
        ("hget", |s, k| s.hget(k, "f").map(drop)),
        ("hdel", |s, k| s.hdel(k, ["f"]).map(drop)),
        ("hexists", |s, k| s.hexists(k, "f").map(drop)),
        ("hlen", |s, k| s.hlen(k).map(drop)),
        ("hgetall", |s, k| s.hgetall(k).map(drop)),
        ("hkeys", |s, k| s.hkeys(k).map(drop)),
    ]
}

fn set_cmds() -> Vec<(&'static str, Cmd)> {
    vec![
        ("sadd", |s, k| s.sadd(k, ["x"]).map(drop)),
        ("srem", |s, k| s.srem(k, ["m"]).map(drop)),
        ("sismember", |s, k| s.sismember(k, "m").map(drop)),
        ("scard", |s, k| s.scard(k).map(drop)),
        ("smembers", |s, k| s.smembers(k).map(drop)),
    ]
}

#[test]
fn every_command_returns_wrong_type_on_every_other_type_and_changes_nothing() {
    let groups = [
        (STR, string_cmds()),
        (LIST, list_cmds()),
        (HASH, hash_cmds()),
        (SET, set_cmds()),
    ];
    for (own, cmds) in &groups {
        for (name, cmd) in cmds {
            for other in [STR, LIST, HASH, SET] {
                let mut s = mixed();
                let before = snapshot(&s);
                let result = cmd(&mut s, other);
                if other == *own {
                    assert!(result.is_ok(), "{name} on its own type {own}: {result:?}");
                } else {
                    assert_eq!(
                        result,
                        Err(Error::WrongType),
                        "{name} on {other} key should be WrongType"
                    );
                    assert_eq!(snapshot(&s), before, "{name} on {other} changed the store");
                }
            }
        }
    }
}

#[test]
fn failed_counter_on_non_integer_string_leaves_store_unchanged() {
    let mut s = mixed();
    s.set("text", "abc");
    s.set("max", i64::MAX.to_string());
    let before = snapshot(&s);
    assert_eq!(s.incr("text"), Err(Error::NotAnInteger));
    assert_eq!(s.incr("max"), Err(Error::Overflow));
    assert_eq!(s.incrby("max", 1), Err(Error::Overflow));
    assert_eq!(snapshot(&s), before);
}

#[test]
fn set_overwrites_a_list_hash_and_set_with_a_string() {
    for key in [LIST, HASH, SET] {
        let mut s = mixed();
        s.set(key, "new");
        assert_eq!(s.get(key).unwrap(), Some(&b"new"[..]), "{key}");
        assert_eq!(s.len(), 4);
    }
    let mut s = mixed();
    s.set(LIST, "new");
    assert_eq!(s.llen(LIST), Err(Error::WrongType));
}

#[test]
fn del_exists_keys_len_work_on_a_mixed_keyspace() {
    let mut s = mixed();
    let all: HashSet<Vec<u8>> = [STR, LIST, HASH, SET]
        .iter()
        .map(|k| k.as_bytes().to_vec())
        .collect();
    assert_eq!(s.len(), 4);
    assert_eq!(key_set(&s), all);
    for k in [STR, LIST, HASH, SET] {
        assert!(s.exists(k));
    }
    assert!(!s.exists("missing"));
    assert!(!s.del("missing"));
    for (i, k) in [STR, LIST, HASH, SET].into_iter().enumerate() {
        assert!(s.del(k), "{k}");
        assert!(!s.exists(k));
        assert!(!s.del(k));
        assert_eq!(s.len(), 3 - i);
        assert!(!key_set(&s).contains(k.as_bytes()));
    }
    assert!(s.keys().is_empty());
}

#[test]
fn emptied_list_hash_and_set_disappear_from_keyspace() {
    let mut s = mixed();
    assert_eq!(s.lpop(LIST).unwrap(), Some(b"a".to_vec()));
    assert!(s.exists(LIST));
    assert_eq!(s.rpop(LIST).unwrap(), Some(b"b".to_vec()));
    assert!(!s.exists(LIST));
    assert_eq!(s.len(), 3);

    assert_eq!(s.hdel(HASH, ["f"]).unwrap(), 1);
    assert!(!s.exists(HASH));
    assert_eq!(s.len(), 2);

    assert_eq!(s.srem(SET, ["m"]).unwrap(), 1);
    assert!(!s.exists(SET));
    assert_eq!(s.len(), 1);

    assert_eq!(key_set(&s), [STR.as_bytes().to_vec()].into_iter().collect());
    // The vanished keys are reusable as any type.
    s.set(LIST, "now a string");
    assert_eq!(s.get(LIST).unwrap(), Some(&b"now a string"[..]));
}

#[test]
fn emptying_by_every_route_removes_the_key() {
    let mut s = Store::new();
    s.lpush("l", ["a"]).unwrap();
    s.rpop("l").unwrap();
    s.rpush("l2", ["a"]).unwrap();
    s.lpop("l2").unwrap();
    s.hset("h", "f", "v").unwrap();
    s.hdel("h", ["f", "nope"]).unwrap();
    s.sadd("t", ["a", "b"]).unwrap();
    s.srem("t", ["a", "b", "c"]).unwrap();
    assert_eq!(s.len(), 0);
    assert!(s.keys().is_empty());
}

#[test]
fn set_overwrites_a_set_and_last_member_removal_deletes_key() {
    // Mirrors the cases in tests/sets.rs; the spec files them under keyspace.
    let mut s = Store::new();
    s.sadd("k", ["a"]).unwrap();
    s.set("k", "v");
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));

    s.sadd("t", ["a", "b"]).unwrap();
    assert_eq!(s.srem("t", ["a", "b"]).unwrap(), 2);
    assert!(!s.exists("t"));
    assert!(!key_set(&s).contains(&b"t".to_vec()));
    assert_eq!(s.len(), 1);
}
