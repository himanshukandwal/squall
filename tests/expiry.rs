use squall::{Clock, ManualClock, Store, SystemClock, Ttl};
use std::time::Duration;

const MS: Duration = Duration::from_millis(1);
const SEC: Duration = Duration::from_secs(1);

fn store() -> (Store, ManualClock) {
    let clock = ManualClock::new();
    (Store::with_clock(clock.clone()), clock)
}

#[test]
fn key_is_live_just_before_its_deadline_and_gone_at_it() {
    let (mut s, clock) = store();
    s.set("k", "v");
    assert!(s.expire("k", 10 * SEC));
    clock.advance(10 * SEC - MS);
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
    assert!(s.exists("k"));
    clock.advance(MS);
    assert_eq!(s.get("k").unwrap(), None);
    assert!(!s.exists("k"));
}

#[test]
fn get_on_expired_key_of_another_type_is_none_not_wrongtype() {
    let (mut s, clock) = store();
    s.rpush("l", ["a"]).unwrap();
    s.expire("l", SEC);
    clock.advance(SEC);
    assert_eq!(s.get("l").unwrap(), None);
}

#[test]
fn ttl_missing_persistent_remaining() {
    let (mut s, clock) = store();
    assert_eq!(s.ttl("nope"), Ttl::Missing);
    s.set("k", "v");
    assert_eq!(s.ttl("k"), Ttl::Persistent);
    s.expire("k", 10 * SEC);
    assert_eq!(s.ttl("k"), Ttl::Remaining(10 * SEC));
    clock.advance(3 * SEC + 250 * MS);
    assert_eq!(s.ttl("k"), Ttl::Remaining(6 * SEC + 750 * MS));
    clock.advance(6 * SEC + 750 * MS);
    assert_eq!(s.ttl("k"), Ttl::Missing);
}

#[test]
fn expire_missing_key_returns_false_and_creates_nothing() {
    let (mut s, _clock) = store();
    assert!(!s.expire("nope", SEC));
    assert!(!s.exists("nope"));
    assert_eq!(s.ttl("nope"), Ttl::Missing);
    assert_eq!(s.len(), 0);
}

#[test]
fn expire_on_expired_key_returns_false() {
    let (mut s, clock) = store();
    s.set("k", "v");
    s.expire("k", SEC);
    clock.advance(SEC);
    assert!(!s.expire("k", 10 * SEC));
    assert_eq!(s.ttl("k"), Ttl::Missing);
}

#[test]
fn expire_replaces_deadline_shorter_or_longer() {
    let (mut s, clock) = store();
    s.set("k", "v");
    assert!(s.expire("k", 10 * SEC));
    assert!(s.expire("k", 20 * SEC));
    assert_eq!(s.ttl("k"), Ttl::Remaining(20 * SEC));
    clock.advance(15 * SEC);
    assert!(s.exists("k"));
    assert!(s.expire("k", 2 * SEC));
    assert_eq!(s.ttl("k"), Ttl::Remaining(2 * SEC));
    clock.advance(2 * SEC);
    assert!(!s.exists("k"));
}

#[test]
fn shortened_deadline_is_the_only_one_drained() {
    let (mut s, clock) = store();
    s.set("k", "v");
    s.expire("k", SEC);
    s.expire("k", 10 * SEC);
    clock.advance(SEC);
    // The replaced deadline no longer applies.
    assert_eq!(s.purge_expired(), 0);
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
}

#[test]
fn expire_zero_deletes_the_key_at_once() {
    let (mut s, _clock) = store();
    s.set("k", "v");
    assert!(s.expire("k", Duration::ZERO));
    assert!(!s.exists("k"));
    assert_eq!(s.ttl("k"), Ttl::Missing);
    assert_eq!(s.len(), 0);
    assert!(!s.expire("k", Duration::ZERO));
}

#[test]
fn expire_overflowing_instant_means_no_ttl() {
    let (mut s, clock) = store();
    s.set("k", "v");
    assert!(s.expire("k", Duration::MAX));
    assert_eq!(s.ttl("k"), Ttl::Persistent);
    // It also replaces an earlier, finite deadline.
    s.expire("k", SEC);
    assert!(s.expire("k", Duration::MAX));
    assert_eq!(s.ttl("k"), Ttl::Persistent);
    clock.advance(SEC);
    assert!(s.exists("k"));
}

#[test]
fn persist_removes_ttl() {
    let (mut s, clock) = store();
    s.set("k", "v");
    s.expire("k", SEC);
    assert!(s.persist("k"));
    assert_eq!(s.ttl("k"), Ttl::Persistent);
    clock.advance(SEC);
    assert_eq!(s.get("k").unwrap(), Some(&b"v"[..]));
    assert_eq!(s.purge_expired(), 0);
}

#[test]
fn persist_false_for_missing_persistent_and_expired_keys() {
    let (mut s, clock) = store();
    assert!(!s.persist("nope"));
    s.set("k", "v");
    assert!(!s.persist("k"));
    s.set("e", "v");
    s.expire("e", SEC);
    clock.advance(SEC);
    assert!(!s.persist("e"));
    assert!(!s.exists("e"));
}

#[test]
fn purge_expired_returns_count_of_removed_keys() {
    let (mut s, clock) = store();
    assert_eq!(s.purge_expired(), 0);
    s.set("a", "1");
    s.set("b", "2");
    s.set("c", "3");
    s.set("p", "4");
    s.expire("a", SEC);
    s.expire("b", SEC);
    s.expire("c", 5 * SEC);
    clock.advance(SEC);
    assert_eq!(s.purge_expired(), 2);
    assert_eq!(s.purge_expired(), 0);
    clock.advance(4 * SEC);
    assert_eq!(s.purge_expired(), 1);
    assert_eq!(s.get("p").unwrap(), Some(&b"4"[..]));
}

#[test]
fn every_mut_command_drains_due_keys_first() {
    let (mut s, clock) = store();
    s.set("a", "1");
    s.set("b", "2");
    s.expire("a", SEC);
    s.expire("b", SEC);
    clock.advance(SEC);
    // A write to an unrelated Key removes both Expired keys.
    s.set("other", "x");
    assert_eq!(s.purge_expired(), 0);

    s.expire("other", SEC);
    clock.advance(SEC);
    s.lpush("l", ["x"]).unwrap();
    assert_eq!(s.purge_expired(), 0);
}

#[test]
fn reads_never_remove_expired_keys() {
    let (mut s, clock) = store();
    s.set("k", "v");
    s.expire("k", SEC);
    clock.advance(SEC);
    assert_eq!(s.get("k").unwrap(), None);
    assert!(!s.exists("k"));
    assert_eq!(s.ttl("k"), Ttl::Missing);
    assert_eq!(s.purge_expired(), 1);
}

#[test]
fn erroring_mut_command_still_drains() {
    let (mut s, clock) = store();
    s.set("e", "v");
    s.expire("e", SEC);
    s.set("str", "x");
    clock.advance(SEC);
    assert!(s.lpush("str", ["a"]).is_err());
    assert_eq!(s.purge_expired(), 0);
}

#[test]
fn del_on_expired_key_returns_false() {
    let (mut s, clock) = store();
    s.set("k", "v");
    s.expire("k", SEC);
    clock.advance(SEC);
    assert!(!s.del("k"));
}

#[test]
fn del_drops_ttl_so_recreated_key_is_persistent() {
    let (mut s, clock) = store();
    s.set("k", "v");
    s.expire("k", SEC);
    assert!(s.del("k"));
    assert_eq!(s.ttl("k"), Ttl::Missing);
    s.rpush("k", ["a"]).unwrap();
    assert_eq!(s.ttl("k"), Ttl::Persistent);
    clock.advance(SEC);
    assert!(s.exists("k"));
    assert_eq!(s.purge_expired(), 0);
}

#[test]
fn emptied_container_drops_ttl() {
    let (mut s, clock) = store();
    s.rpush("l", ["a"]).unwrap();
    s.expire("l", SEC);
    s.lpop("l").unwrap();
    assert_eq!(s.ttl("l"), Ttl::Missing);
    s.hset("l", "f", "v").unwrap();
    assert_eq!(s.ttl("l"), Ttl::Persistent);
    clock.advance(SEC);
    assert!(s.exists("l"));
    assert_eq!(s.purge_expired(), 0);
}

#[test]
fn manual_clock_clones_share_time() {
    let a = ManualClock::new();
    let b = a.clone();
    let start = a.now();
    b.advance(5 * SEC);
    assert_eq!(a.now(), start + 5 * SEC);
    assert_eq!(b.now(), a.now());
}

#[test]
fn store_with_any_clock_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Store>();
    assert_send_sync::<ManualClock>();
    assert_send_sync::<SystemClock>();
    let _ = Store::with_clock(SystemClock);
    let _ = Store::default();
}

#[test]
fn store_debug_does_not_require_clock_debug() {
    struct Fixed(std::time::Instant);
    impl Clock for Fixed {
        fn now(&self) -> std::time::Instant {
            self.0
        }
    }
    let mut s = Store::with_clock(Fixed(std::time::Instant::now()));
    s.set("k", "v");
    assert!(format!("{s:?}").contains("Store"));
}
