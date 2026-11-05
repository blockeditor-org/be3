use std::time::Instant;

use crate::store::passwords::{
    EMAIL_SLOWDOWN, FAILURE_DECAY, FREE_FAILURES, FREE_FAILURES_PER_ADDRESS,
    FREE_FAILURES_PER_EMAIL, LONGEST_LOCKOUT, MAX_FAILURE_RECORDS, MAX_KNOWN_EMAILS, Passwords,
};

#[test]
fn sign_in_failures_lock_a_pair_before_an_address_or_an_email_and_decay() {
    let passwords = Passwords::default();
    let start = Instant::now();
    for _ in 0..FREE_FAILURES {
        assert!(
            passwords
                .locked_out("owner@example.com", "192.0.2.1", false, start)
                .is_none()
        );
        passwords.failed("owner@example.com", "192.0.2.1", start);
    }
    assert!(
        passwords
            .locked_out("owner@example.com", "192.0.2.1", false, start)
            .is_some()
    );
    // The owner, elsewhere, is not locked out by a stranger's failures.
    assert!(
        passwords
            .locked_out("owner@example.com", "198.51.100.7", false, start)
            .is_none()
    );

    // Failures are forgotten as time passes: after the lockout and some
    // quiet, one more failure doesn't lock the pair out again.
    let later = start + LONGEST_LOCKOUT + FAILURE_DECAY * 2;
    assert!(
        passwords
            .locked_out("owner@example.com", "192.0.2.1", false, later)
            .is_none()
    );
    passwords.failed("owner@example.com", "192.0.2.1", later);
    assert!(
        passwords
            .locked_out("owner@example.com", "192.0.2.1", false, later)
            .is_none()
    );

    // One address trying many emails is locked out for all of them.
    for index in 0..FREE_FAILURES_PER_ADDRESS {
        passwords.failed(&format!("guess{index}@example.com"), "203.0.113.5", start);
    }
    assert!(
        passwords
            .locked_out("fresh@example.com", "203.0.113.5", false, start)
            .is_some()
    );

    // Many addresses against one email only slow it down, and never for an
    // address that has signed in to it or a signed-in browser.
    passwords.succeeded("target@example.com", "2001:db8:1::1", start);
    for index in 0..FREE_FAILURES_PER_EMAIL {
        passwords.failed(
            "target@example.com",
            &format!("10.1.{}.{}", index / 200, index % 200),
            start,
        );
    }
    assert!(
        passwords
            .locked_out("target@example.com", "198.51.100.7", false, start)
            .is_some_and(|wait| wait <= EMAIL_SLOWDOWN)
    );
    let more = start + EMAIL_SLOWDOWN * 10;
    passwords.failed("target@example.com", "10.1.9.9", more);
    assert!(
        passwords
            .locked_out("target@example.com", "198.51.100.7", false, more)
            .is_some_and(|wait| wait <= EMAIL_SLOWDOWN)
    );
    assert!(
        passwords
            .locked_out("target@example.com", "2001:db8:1::2", false, more)
            .is_none()
    );
    assert!(
        passwords
            .locked_out("target@example.com", "198.51.100.7", true, more)
            .is_none()
    );
    passwords.forget_email("target@example.com");
    assert!(
        passwords
            .locked_out("target@example.com", "198.51.100.7", false, start)
            .is_none()
    );

    // However many emails and addresses fail, the records stay bounded.
    let mut index = 0u32;
    while passwords.records() < MAX_FAILURE_RECORDS {
        passwords.failed(
            &format!("many{index}@example.com"),
            &format!("10.2.{}.{}", index / 250, index % 250),
            start,
        );
        index += 1;
    }
    for extra in 0..10 {
        passwords.failed(&format!("more{extra}@example.com"), "10.3.0.1", start);
    }
    assert!(passwords.records() <= MAX_FAILURE_RECORDS);
    for index in 0..MAX_KNOWN_EMAILS + 10 {
        passwords.succeeded(&format!("known{index}@example.com"), "192.0.2.1", start);
    }
    assert!(passwords.known_emails() <= MAX_KNOWN_EMAILS);
}
