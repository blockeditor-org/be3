use super::*;

use be_block_graph::Access;

/// Every entry of a log carries when the server took it, which its author cannot choose, so a
/// contact's entry is judged by it: one the replaced key signed that the server took after this
/// device learned of the new key answers `Replaced`, one it signed before answers `Changed`, and
/// one the new key signed is `Verified`.
#[tokio::test]
async fn an_entry_signed_with_a_replaced_key_after_its_replacement_answers_replaced() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let team = workspace(&owner, "Team").await;
    let public = owner.verified_identity(guest.account()).unwrap();
    owner
        .share(team, guest.account(), Access::Edit, &public)
        .await
        .unwrap();
    let before = owner.append(team, b"before the new phrase").await.unwrap();
    assert!(matches!(before, Appended::Logged { seq: 0, .. }));
    let old = owner.keys().identity().clone();
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    take_phrase(&owner.connection(), owner.account(), &old, &fresh).await;
    guest.refresh_contacts().await.unwrap();
    let now = guest.server_now().expect("the guest measured the server's clock");

    let owner = Peer::connect(
        PeerConfig::new(
            &harness.url,
            fresh,
            Credentials::Token(owner.token().to_owned()),
        )
        .organization(Some(owner.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    owner.append(team, b"after, with the new key").await.unwrap();
    // The owner's phone, never told of the new phrase, still signs with the old key, and the
    // server takes its entry two minutes after the guest learned of the new one.
    let payload = b"sealed by the phone".to_vec();
    let id = Uuid::new_v4();
    let signature = old.sign_authored(
        team.into_bytes(),
        &be_block_protocol::log_signed(id, be_block_store::Hash::of(&payload)),
    );
    let database = harness.database();
    be_server::log::append(
        &database,
        owner.organization(),
        team,
        owner.account(),
        vec![be_block_protocol::NewEntry {
            id,
            payload,
            signature,
        }],
        now + 2 * 60 * 1000,
    )
    .unwrap();
    drop(database);

    let read = guest.read_log(team, 0, 10).await.unwrap();
    assert_eq!(read.head.size, 3);
    let mut signers = Vec::new();
    for logged in &read.entries {
        signers.push(guest.verify_entry(team, &logged.entry).await.unwrap());
    }
    assert_eq!(
        signers,
        [Signer::Changed, Signer::Verified, Signer::Replaced],
        "the old key before the change, the new key, the old key after it"
    );
    assert_eq!(
        read.entries[1].opened.as_deref(),
        Some(&b"after, with the new key"[..])
    );
    harness.stop().await;
}
