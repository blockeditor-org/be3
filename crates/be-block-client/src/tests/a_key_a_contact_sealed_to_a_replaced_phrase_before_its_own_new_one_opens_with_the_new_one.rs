use super::*;

use be_block_graph::{Access, PREFIXES_TYPE};
use be_type_workspace::FolderContent;

/// As `a_key_sealed_to_a_phrase_the_reader_replaced_opens_with_the_new_one`, but the admin,
/// which still has not followed the guest's key change, then takes its own new phrase, sealing
/// its key change to the guest's old key. The kind's key it sealed from its old key to the
/// guest's old key opens by neither of the admin's keys a device of the guest's then holds, so
/// it reaches the guest's new phrase only as the device follows the admin's change and seals
/// again what the admin's old key sealed (`contacts::reseal_from`), with the guest's retired
/// secret.
#[tokio::test]
async fn a_key_a_contact_sealed_to_a_replaced_phrase_before_its_own_new_one_opens_with_the_new_one()
{
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
    // The guest's key change never reaches the admin.
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
    // Its change goes to the guest's old key, the one its record holds.
    new_phrase(
        &admin.connection(),
        admin.account(),
        admin.keys().identity(),
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
    let contacts = renewed.refresh_contacts().await.unwrap();
    assert!(
        contacts
            .iter()
            .any(|contact| contact.account == admin.account() && contact.public.is_some()),
        "the guest did not follow the admin's key change sealed to its old key: {contacts:?}"
    );
    renewed.refresh_keys(None).await.unwrap();
    assert_eq!(
        renewed.keys().scope_key(kind),
        Some(key),
        "the guest's new phrase did not open the key the admin's old key sealed to its old one"
    );

    harness.stop().await;
}
