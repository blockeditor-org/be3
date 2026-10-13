use super::*;

use be_block::be_model::{Anchor, ObjectId};
use be_block::{Checklist, ChecklistContent, ChecklistItem};

#[tokio::test]
async fn a_late_joiner_places_an_insert_after_an_item_removed_before_it_joined() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<ChecklistContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &ChecklistContent::default(), None)
        .await
        .unwrap();
    let owner_peer = Arc::new(first);
    let joiner_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, ChecklistContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    let mut removed = ObjectId::ROOT;
    for text in ["a", "b", "c"] {
        let (id, edit) = Checklist::add(text);
        if text == "b" {
            removed = id;
        }
        owner.edit(edit).await.unwrap();
    }
    owner.edit(Checklist::remove(removed)).await.unwrap();
    owner.seal().await.unwrap();

    let mut joiner = Live::<_, ChecklistContent>::join(Arc::clone(&joiner_peer), block)
        .await
        .unwrap();
    until(
        &mut [&mut owner, &mut joiner],
        "caught the joiner up",
        |sessions| sessions[1].content().session_state() == sessions[0].content().session_state(),
    )
    .await;
    let (_, insert) = Checklist::ITEMS.insert(
        ObjectId::ROOT,
        Anchor::After(removed),
        &ChecklistItem {
            text: "d".into(),
            done: false,
        },
    );
    owner.edit(insert.into()).await.unwrap();
    until(
        &mut [&mut owner, &mut joiner],
        "delivered the insert",
        |sessions| checklist_texts(sessions[1]).len() == 3,
    )
    .await;

    assert_eq!(checklist_texts(&owner), ["a", "d", "c"]);
    assert_eq!(checklist_texts(&joiner), ["a", "d", "c"]);

    harness.stop().await;
}
