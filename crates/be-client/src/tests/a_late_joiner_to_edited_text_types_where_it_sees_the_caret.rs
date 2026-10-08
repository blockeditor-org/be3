use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn a_late_joiner_to_edited_text_types_where_it_sees_the_caret() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextBlock::of("one\n"), None)
        .await
        .unwrap();
    let owner_peer = Arc::new(first);
    let joiner_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, TextContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    type_at(&mut owner, 4, "two\n").await;
    owner.seal().await.unwrap();
    type_at(&mut owner, 8, "three\n").await;
    erase(&mut owner, 0..4).await;

    let mut joiner = Live::<_, TextContent>::join(Arc::clone(&joiner_peer), block)
        .await
        .unwrap();
    until(
        &mut [&mut owner, &mut joiner],
        "caught the joiner up",
        |sessions| sessions[1].content().to_text() == "two\nthree\n",
    )
    .await;
    type_at(&mut joiner, 4, "and a half\n").await;
    until(&mut [&mut owner, &mut joiner], "converged", |sessions| {
        sessions[0].content().to_text() == sessions[1].content().to_text() && sessions[1].is_clean()
    })
    .await;

    assert_eq!(owner.content().to_text(), "two\nand a half\nthree\n");
    harness.stop().await;
}
