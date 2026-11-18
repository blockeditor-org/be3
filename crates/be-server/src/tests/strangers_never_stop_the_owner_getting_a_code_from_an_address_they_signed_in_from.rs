use super::*;
use crate::store::codes::{self, CodePurpose, CodeRequest};

fn sign_in<'a>(email: &'a str, attempt: &'a str, address: &'a str) -> CodeRequest<'a> {
    CodeRequest {
        purpose: CodePurpose::SignIn,
        email,
        account: None,
        display_name: "",
        invite: "",
        device_code: "",
        attempt,
        address,
    }
}

#[tokio::test]
async fn strangers_never_stop_the_owner_getting_a_code_from_an_address_they_signed_in_from() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    // The owner signs in once from home, which remembers the address for the account.
    let home = "2001:db8:1:2::7";
    let attempt = crate::new_secret();
    let mailed = store
        .request_code(sign_in("owner@example.com", &attempt, home))
        .await
        .unwrap()
        .unwrap();
    store
        .enter_code(
            &attempt,
            &mailed.code,
            "",
            home,
            |_, _| true,
            ServerConfig::OPEN,
        )
        .await
        .unwrap();
    // Strangers, each from a network of their own, spend the account's budget.
    let mut refused = false;
    for stranger in 0..codes::CODES_PER_ACCOUNT + 1 {
        let address = format!("2001:db8:{:x}::1", 0x100 + stranger);
        let attempt = crate::new_secret();
        match store
            .request_code(sign_in("owner@example.com", &attempt, &address))
            .await
        {
            Ok(mailed) => assert!(mailed.is_some()),
            Err(_) => refused = true,
        }
    }
    assert!(refused, "strangers are held to the account's budget");
    // The owner still gets one from home.
    let attempt = crate::new_secret();
    let mailed = store
        .request_code(sign_in("owner@example.com", &attempt, home))
        .await
        .unwrap();
    assert!(mailed.is_some());
    // An address is capped by its /64, and a network by its /48, whatever email it asks for.
    let mut held = 0;
    for request in 0..codes::CODES_PER_NETWORK + 1 {
        let address = format!("2001:db8:ffff:{:x}::1", request / codes::CODES_PER_ADDRESS);
        let attempt = crate::new_secret();
        if store
            .request_code(sign_in(
                &format!("{request}@example.com"),
                &attempt,
                &address,
            ))
            .await
            .is_err()
        {
            held += 1;
        }
    }
    assert!(held > 0, "a /48 spreading over its /64s is still capped");
}
