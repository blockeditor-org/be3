use super::*;

use be_block::be_model::{Anchor, ObjectId};
use be_block::{Checklist, ChecklistContent, ChecklistItem, LiveEdit};

#[tokio::test]
async fn a_quiet_session_restarts_and_its_followers_forget_its_removals() {
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
    let follower_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, ChecklistContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    let mut follower = Live::<_, ChecklistContent>::join(Arc::clone(&follower_peer), block)
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
    until(
        &mut [&mut owner, &mut follower],
        "removed the item",
        |sessions| !sessions[1].content().session_state().is_empty(),
    )
    .await;
    owner.seal().await.unwrap();

    assert!(owner.activity().is_some());
    assert!(owner.restart().await.unwrap());
    assert_eq!(owner.activity(), None);
    assert!(owner.content().session_state().is_empty());
    until(&mut [&mut owner, &mut follower], "restarted", |sessions| {
        sessions[1].content().session_state().is_empty()
    })
    .await;
    let (_, insert) = Checklist::ITEMS.insert(
        ObjectId::ROOT,
        Anchor::After(removed),
        &ChecklistItem {
            text: "d".into(),
            done: false,
        },
    );
    follower.edit(insert.into()).await.unwrap();
    until(
        &mut [&mut owner, &mut follower],
        "accepted the insert",
        |sessions| checklist_texts(sessions[0]).len() == 3 && sessions[1].is_clean(),
    )
    .await;

    assert_eq!(checklist_texts(&owner), ["a", "c", "d"]);
    assert_eq!(checklist_texts(&follower), ["a", "c", "d"]);

    harness.stop().await;
}
