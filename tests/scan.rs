use squall::Store;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

fn scan(s: &Store, pat: &str) -> BTreeSet<Vec<u8>> {
    s.scan(pat).into_iter().collect()
}

fn store(keys: &[&str]) -> Store {
    let mut s = Store::new();
    for k in keys {
        s.set(k, "v");
    }
    s
}

fn set_of(keys: &[&str]) -> BTreeSet<Vec<u8>> {
    keys.iter().map(|k| k.as_bytes().to_vec()).collect()
}

#[test]
fn star_matches_any_run_including_none() {
    let s = store(&["user:1", "user:", "admin:1", "xuser:1"]);
    assert_eq!(scan(&s, "*"), set_of(&["user:1", "user:", "admin:1", "xuser:1"]));
    assert_eq!(scan(&s, "user:*"), set_of(&["user:1", "user:"]));
    assert_eq!(scan(&s, "*:1"), set_of(&["user:1", "admin:1", "xuser:1"]));
    assert_eq!(scan(&s, "u*r:*"), set_of(&["user:1", "user:"]));
}

#[test]
fn question_mark_matches_exactly_one_byte() {
    let s = store(&["a", "ab", "abc", ""]);
    assert_eq!(scan(&s, "?"), set_of(&["a"]));
    assert_eq!(scan(&s, "a?"), set_of(&["ab"]));
    assert_eq!(scan(&s, "???"), set_of(&["abc"]));
}

#[test]
fn bracket_class_ranges_and_negation() {
    let s = store(&["ha", "hb", "hc", "hd", "h1"]);
    assert_eq!(scan(&s, "h[ab]"), set_of(&["ha", "hb"]));
    assert_eq!(scan(&s, "h[a-c]"), set_of(&["ha", "hb", "hc"]));
    assert_eq!(scan(&s, "h[c-a]"), set_of(&["ha", "hb", "hc"]));
    assert_eq!(scan(&s, "h[^a]"), set_of(&["hb", "hc", "hd", "h1"]));
    assert_eq!(scan(&s, "h[^a-c]"), set_of(&["hd", "h1"]));
    assert_eq!(scan(&s, "h[a-c1]"), set_of(&["ha", "hb", "hc", "h1"]));
}

#[test]
fn backslash_escapes_specials() {
    let s = store(&["a*b", "axb", "a?b", "a[b", "a\\b"]);
    assert_eq!(scan(&s, "a\\*b"), set_of(&["a*b"]));
    assert_eq!(scan(&s, "a\\?b"), set_of(&["a?b"]));
    assert_eq!(scan(&s, "a\\[b"), set_of(&["a[b"]));
    assert_eq!(scan(&s, "a\\\\b"), set_of(&["a\\b"]));
    assert_eq!(scan(&s, "a[\\]]b"), set_of(&[]));
}

#[test]
fn matches_keys_of_every_type() {
    let mut s = Store::new();
    s.set("k:string", "v");
    s.lpush("k:list", ["a"]).unwrap();
    s.sadd("k:set", ["a"]).unwrap();
    s.hset("k:hash", "f", "v").unwrap();
    assert_eq!(
        scan(&s, "k:*"),
        set_of(&["k:string", "k:list", "k:set", "k:hash"])
    );
}

#[test]
fn case_sensitive_and_binary_safe() {
    let mut s = Store::new();
    s.set("Abc", "v");
    s.set("abc", "v");
    s.set(b"a\0b".to_vec(), "v");
    s.set(b"\xff\xfe".to_vec(), "v");
    assert_eq!(scan(&s, "abc"), set_of(&["abc"]));
    assert_eq!(scan(&s, "A*"), set_of(&["Abc"]));
    let nul: BTreeSet<Vec<u8>> = s.scan(b"a?b").into_iter().collect();
    assert_eq!(nul, BTreeSet::from([b"a\0b".to_vec()]));
    let hi: BTreeSet<Vec<u8>> = s.scan(b"\xff?").into_iter().collect();
    assert_eq!(hi, BTreeSet::from([b"\xff\xfe".to_vec()]));
    let class: BTreeSet<Vec<u8>> = s.scan(b"[\xff]\xfe").into_iter().collect();
    assert_eq!(class, BTreeSet::from([b"\xff\xfe".to_vec()]));
}

#[test]
fn empty_pattern_matches_only_empty_key() {
    let mut s = store(&["a"]);
    assert!(s.scan("").is_empty());
    s.set("", "v");
    assert_eq!(scan(&s, ""), set_of(&[""]));
}

#[test]
fn malformed_patterns_do_not_error() {
    let s = store(&["a", "ab", "[", "[a", "a\\"]);
    // Unterminated class: Redis treats it as ending at pattern end.
    assert_eq!(scan(&s, "[a"), set_of(&["a"]));
    assert_eq!(scan(&s, "[^b"), set_of(&["a", "["]));
    // Trailing backslash matches a literal backslash.
    assert_eq!(scan(&s, "a\\"), set_of(&["a\\"]));
    // Degenerate classes must not panic.
    let _ = s.scan("[]");
    let _ = s.scan("[");
    let _ = s.scan("[^");
}

#[test]
fn pathological_pattern_finishes_promptly() {
    let mut s = Store::new();
    s.set("a".repeat(5000), "v");
    let start = Instant::now();
    assert!(s.scan("*a*a*a*a*a*a*a*b").is_empty());
    assert!(start.elapsed() < Duration::from_secs(2));
}
