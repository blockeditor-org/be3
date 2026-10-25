use super::*;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn removing_a_member_shuts_them_out_and_rotates_what_they_could_read() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let removed = harness.member(&admin, "removed@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let before = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    for member in [&reader, &removed] {
        let public = admin.keys().contact(member.account()).unwrap();
        admin
            .share(team, member.account(), Access::View, &public)
            .await
            .unwrap();
    }
    assert_eq!(text(&removed, note).await, "before");

    admin.remove_member(removed.account()).await.unwrap();
    assert!(
        removed
            .connection()
            .request(|request| be_protocol::ClientMessage::ReadBlock {
                request,
                block: note
            })
            .await
            .is_err(),
        "a removed member's open connection kept reading"
    );
    let again = Peer::connect(
        PeerConfig::new(
            &harness.url,
            removed.keys().identity().clone(),
            Credentials::Token(removed.token().to_owned()),
        )
        .organization(Some(admin.organization())),
        MemoryStore::new(),
    )
    .await;
    assert!(again.is_err(), "a removed member opened the organization");
    let members = admin.members().await.unwrap();
    assert!(
        members
            .iter()
            .all(|member| member.account != removed.account())
    );
    let mut rotations = admin.key_work().await.unwrap().rotations;
    rotations.sort();
    let mut expected = vec![team, admin.organization_settings()];
    expected.sort();
    assert_eq!(rotations, expected);

    admin.tend().await.unwrap();
    admin
        .save(note, &TextBlock::of("after"), before)
        .await
        .unwrap();
    assert_eq!(text(&reader, note).await, "after");

    harness.stop().await;
}
