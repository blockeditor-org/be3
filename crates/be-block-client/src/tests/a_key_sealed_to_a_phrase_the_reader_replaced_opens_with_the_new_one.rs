use super::*;

use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::FolderContent;

/// The owner shares a folder with the guest and a reader. The guest takes a new phrase; the
/// owner, which has not yet followed the guest's key change, removes the reader, and the
/// folder's new key is sealed to the guest's old key. A device of the guest's on the new phrase
/// reads what is written under the new key all the same, with the secret the guest's block
/// keeps from the old phrase (`AccountBlock::retired`, `KeyBook::retire`).
#[tokio::test]
async fn a_key_sealed_to_a_phrase_the_reader_replaced_opens_with_the_new_one() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let reader = harness.member(&owner, "reader@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let folder = owner
        .create::<FolderContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    for (peer, access) in [(&guest, Access::Edit), (&reader, Access::View)] {
        let public = owner.verified_identity(peer.account()).unwrap();
        owner
            .share(folder, peer.account(), access, &public)
            .await
            .unwrap();
    }
    // The guest's key change reaches the owner only later.
    let from = guest.account();
    harness.server.lie(owner.account(), move |page| {
        if let be_block_protocol::Page::Mailbox(messages) = page {
            messages.retain(|message| {
                !(message.from == Some(from)
                    && matches!(message.letter, be_block_protocol::Letter::KeyChange(_)))
            });
        }
    });
    let fresh = new_phrase(&guest.connection(), guest.account(), guest.keys().identity()).await;

    owner
        .grant(folder, reader.account(), Access::None)
        .await
        .unwrap();
    owner.tend().await.unwrap();
    let note = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("sealed to the old key"), None)
        .await
        .unwrap();

    let renewed = Peer::connect(
        PeerConfig::new(
            &harness.url,
            fresh,
            Credentials::Token(guest.token().to_owned()),
        )
        .organization(Some(guest.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    renewed.refresh_contacts().await.unwrap();
    assert_eq!(
        text(&renewed, note).await,
        "sealed to the old key",
        "the guest's new phrase did not open what was sealed to its old key"
    );

    harness.stop().await;
}
