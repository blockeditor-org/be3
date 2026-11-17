use be_block_graph::SETTINGS_TYPE;

use super::*;

/// A block made inside an owner-only block has keys its author alone reads. Moved out into a
/// shared scope, those keys are wrapped under that scope, so they are no longer the author's
/// alone: escrowed as any shared key, on the device that moved it and on another device of the
/// author's that opens them, whichever wrap it opens them through first.
#[tokio::test]
async fn a_block_moved_out_of_an_owner_only_block_is_escrowed_as_any_shared_one() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let settings = admin
        .create_block(
            Uuid::now_v7(),
            SETTINGS_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .unwrap()
        .id;
    let secret = admin
        .create_block(
            Uuid::now_v7(),
            be_type_text::TextContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            &be_block_content::BlockMetadata::default(),
            true,
        )
        .await
        .unwrap()
        .id;
    let inside = admin
        .create::<be_type_text::TextContent>(BlockParent::Block(secret))
        .await
        .unwrap();
    let made = admin.current_key(inside).expect("the block has a key");
    assert!(
        admin.keys().owner_only_covers(made),
        "a key made inside an owner-only block was not its author's alone"
    );

    let team = workspace(&admin, "Team").await;
    admin
        .set_parent(inside, BlockParent::Block(team))
        .await
        .unwrap();
    let escrow = admin.keys().escrow().expect("the organization has an escrow");
    for (device, peer) in [("moving", &admin), ("other", &another(&harness, &admin).await)] {
        peer.refresh_keys(None).await.unwrap();
        assert!(peer.keys().has(made), "the {device} device holds no moved key");
        assert!(
            !peer.keys().owner_only_covers(made),
            "the {device} device still holds a key moved into a shared scope as owner-only"
        );
        assert!(
            peer.keys()
                .escrow_wrap_under(made, &escrow)
                .unwrap()
                .is_some(),
            "the {device} device would not escrow a key moved into a shared scope"
        );
    }

    harness.stop().await;
}
