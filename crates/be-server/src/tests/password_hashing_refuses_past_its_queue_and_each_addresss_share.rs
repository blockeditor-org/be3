use super::*;
use crate::store::passwords::{
    PASSWORD_CHECKS_AT_ONCE, PASSWORD_CHECKS_PER_ADDRESS, PASSWORD_CHECKS_WAITING, address_key,
};

#[tokio::test]
async fn password_hashing_refuses_past_its_queue_and_each_addresss_share() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let passwords = &store.passwords;
    let mine: Vec<_> = (0..PASSWORD_CHECKS_PER_ADDRESS)
        .map(|_| passwords.admit("2001:db8::1").unwrap())
        .collect();
    assert!(passwords.admit("2001:db8::2").is_err());
    assert_eq!(address_key("2001:db8::2"), address_key("2001:db8::1"));
    assert_eq!(address_key("::ffff:192.0.2.1"), "192.0.2.1");
    assert_eq!(
        address_key("2001:db8::1"),
        "2001:db8:0:0::/64",
        "account_mail stores this spelling"
    );
    let mut others = Vec::new();
    for index in 0..PASSWORD_CHECKS_AT_ONCE + PASSWORD_CHECKS_WAITING - mine.len() {
        others.push(
            passwords
                .admit(&format!("10.0.{}.{}", index / 200, index % 200))
                .unwrap(),
        );
    }
    assert!(passwords.admit("192.0.2.9").is_err());
    drop(mine);
    let place = passwords.admit("192.0.2.9").unwrap();
    drop((place, others));
    assert!(
        store
            .login_browser("nobody@example.com", "a password", "192.0.2.9")
            .await
            .is_err()
    );
}
