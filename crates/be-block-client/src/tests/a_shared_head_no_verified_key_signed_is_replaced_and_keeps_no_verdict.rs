use super::*;
use crate::peer::forks::SharedHead;

async fn append(peer: &Peer<MemoryStore>, block: Uuid, texts: &[&str]) {
    for text in texts {
        peer.append(
            block,
            "test.app",
            text.as_bytes(),
            be_block_content::Along::default(),
        )
        .await
        .unwrap();
    }
    peer.read_log(block, 0, 10).await.unwrap();
}

/// A head in the account's block that no key this device verified signed, such as one signed
/// before the account's phrase changed or one the server put there, is compared with nothing:
/// the pass answers `Forged` for it, keeps no verdict, and shares this device's own head in
/// its place. A head another organization's block of the same id holds is neither compared nor
/// touched, since a block id is unique only within its organization.
#[tokio::test]
async fn a_shared_head_no_verified_key_signed_is_replaced_and_keeps_no_verdict() {
    let harness = Harness::start().await;
    let laptop = harness.owner("owner@example.com").await;
    let organization = laptop.organization();
    let chat = workspace(&laptop, "Chat").await;
    append(&laptop, chat, &["one", "two"]).await;
    let ours = laptop.signed_log_head(chat).unwrap();

    let old = be_block_keys::RecoverySecret::from_bytes([7; 32]);
    let root = [3; 32];
    let signed_with_old = |organization: Uuid| SharedHead {
        size: 1,
        root,
        signature: old.sign_log_head(organization.into_bytes(), chat.into_bytes(), 1, &root),
        updated: 0,
    };
    let elsewhere = Uuid::new_v4();
    laptop
        .put_account_log_head(organization, chat, signed_with_old(organization))
        .await;
    laptop
        .put_account_log_head(elsewhere, chat, signed_with_old(elsewhere))
        .await;

    let compared = laptop.trade_account_heads().await.unwrap();
    assert!(
        matches!(compared.as_slice(), [(block, Err(ClientError::Forged(forged)))]
            if *block == chat && *forged == chat),
        "only this organization's head is compared, and it is refused: {compared:?}"
    );
    assert_eq!(
        laptop.log_fork(chat),
        None,
        "a head no key vouched for keeps no verdict"
    );
    let held = laptop.account_log_head(organization, chat).await.unwrap();
    assert_eq!(
        (held.size, held.root, held.signature),
        (2, *ours.head.root.as_bytes(), ours.signature),
        "the device's own head replaces it"
    );
    assert_eq!(
        laptop.account_log_head(elsewhere, chat).await,
        Some(signed_with_old(elsewhere)),
        "another organization's head is left alone"
    );
    harness.stop().await;
}
