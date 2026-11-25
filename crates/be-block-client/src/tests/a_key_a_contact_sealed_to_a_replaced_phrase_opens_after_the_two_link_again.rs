use super::*;

use be_block_graph::{Access, PREFIXES_TYPE};
use be_type_workspace::FolderContent;

/// As `a_key_sealed_to_a_phrase_the_reader_replaced_opens_with_the_new_one`, the admin, which
/// has not followed the guest's new phrase, offers the guest a kind's key sealed to the guest's
/// old key. Before the device on the new phrase reads it, the two connect again by a link, whose
/// record holds the guest's current key, so nothing the admin seals after opens with a retired
/// secret: what the admin sealed before is sealed again to the new phrase as the link is
/// accepted (`contacts::reseal_relinked`), and opens.
#[tokio::test]
async fn a_key_a_contact_sealed_to_a_replaced_phrase_opens_after_the_two_link_again() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let guest = harness.member(&admin, "guest@example.com").await;
    let team = workspace(&admin, "Team").await;
    let folder = admin
        .create::<FolderContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let old = admin.verified_identity(guest.account()).unwrap();
    admin
        .share(folder, guest.account(), Access::View, &old)
        .await
        .unwrap();
    let from = guest.account();
    harness.server.lie(admin.account(), move |page| {
        if let be_block_protocol::Page::Mailbox(messages) = page {
            messages.retain(|message| {
                !(message.from == Some(from)
                    && matches!(message.letter, be_block_protocol::Letter::KeyChange(_)))
            });
        }
    });
    let fresh = new_phrase(
        &guest.connection(),
        guest.account(),
        guest.keys().identity(),
    )
    .await;
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

    // Sealed after the new phrase's device last read its keys.
    let kind = admin
        .create_block(
            Uuid::now_v7(),
            PREFIXES_TYPE,
            BlockParent::Block(team),
            &be_block_content::BlockMetadata::named("Prefixes"),
            false,
        )
        .await
        .unwrap()
        .id;
    admin.open_inside(kind, Access::View).await.unwrap();
    admin.refresh_contacts().await.unwrap();
    admin.tend().await.unwrap();
    let key = admin.keys().scope_key(kind).unwrap();
    assert_eq!(sealed_to(&harness, key, guest.account()), 1);

    befriend(&admin, &renewed).await;
    renewed
        .keys()
        .absorb(served_wrap(&harness, &admin, key, guest.account()));
    assert_eq!(
        renewed.keys().scope_key(kind),
        Some(key),
        "the key sealed to the old phrase before the link did not open after it"
    );

    harness.stop().await;
}
