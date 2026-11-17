use super::*;

use be_block_protocol::WrappedBy;

/// A new escrow key is this device's escrow only once the server holds it: a block made between
/// minting it and sending it is wrapped under the escrow the server already has, which the server
/// takes, not under the minted one, which it would refuse.
#[tokio::test]
async fn a_block_made_while_an_escrow_key_is_minted_wraps_under_the_one_the_server_holds() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let held = admin
        .keys()
        .escrow()
        .expect("the creator pins its escrow key");
    let (minted, _) = admin
        .keys()
        .mint_escrow(admin.checked_organization().await.unwrap().unwrap().head)
        .unwrap();
    assert_eq!(
        admin.keys().escrow(),
        Some(held.clone()),
        "a minted escrow key waits for the server"
    );

    let made = workspace(&admin, "Made meanwhile").await;
    let scope = admin.keys().scope_key(made).unwrap();
    let wrapped = admin.keys().wrapped_by(scope);
    assert!(wrapped.contains(&WrappedBy::Key(held.id)), "{wrapped:?}");
    assert!(!wrapped.contains(&WrappedBy::Key(minted.id)), "{wrapped:?}");
}
