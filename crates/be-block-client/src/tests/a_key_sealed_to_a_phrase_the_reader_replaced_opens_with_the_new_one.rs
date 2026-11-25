use super::*;

use be_block_graph::{Access, PREFIXES_TYPE};
use be_type_workspace::FolderContent;

/// The admin shares a folder inside a workspace with the guest, who takes a new phrase. The
/// admin, which has not yet followed the guest's key change, opens a kind of the workspace's
/// settings to anyone inside, and key work offers its key to the guest, a contact viewing
/// inside, sealed to the key the admin holds for it: the guest's old one. A grantee's key
/// follows the retirement in each log naming it, but a contact's only its key change. A device
/// of the guest's on the new phrase opens the kind's key all the same, with the secret the
/// guest's block keeps from the old phrase (`AccountBlock::retired`, `KeyBook::retire`), as its
/// record of the admin holds the guest's old key (`OwnContact::holds`), and seals it again to the
/// new one.
#[tokio::test]
async fn a_key_sealed_to_a_phrase_the_reader_replaced_opens_with_the_new_one() {
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
    // The guest's key change reaches the admin only later.
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
    assert_eq!(
        sealed_to(&harness, key, guest.account()),
        1,
        "the kind's key was not offered to the guest viewing inside"
    );
    assert_eq!(
        admin.verified_identity(guest.account()),
        Some(old),
        "the admin followed the guest's key change, so sealed to the new key"
    );

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
    renewed.refresh_keys(None).await.unwrap();
    assert_eq!(
        renewed.keys().scope_key(kind),
        Some(key),
        "the guest's new phrase did not open the key sealed to its old one"
    );
    // Opened with a retired secret once, then sealed again to the new one, from the guest.
    let sender: String = harness
        .database()
        .query_row(
            "SELECT sender FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            [key.to_string(), format!("account:{}", guest.account())],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        sender,
        guest.account().to_string(),
        "the key a retired secret opened was not sealed again to the new phrase"
    );

    harness.stop().await;
}
