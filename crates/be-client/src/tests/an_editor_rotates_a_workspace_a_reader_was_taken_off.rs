use super::*;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn an_editor_rotates_a_workspace_a_reader_was_taken_off() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let editor = harness.member(&owner, "editor@example.com").await;
    let leaver = harness.member(&owner, "leaver@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let before = owner
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    for (member, access) in [(&editor, Access::Edit), (&leaver, Access::View)] {
        let public = owner.keys().contact(member.account()).unwrap();
        owner
            .share(team, member.account(), access, &public)
            .await
            .unwrap();
        assert_eq!(text(member, note).await, "before");
    }
    let old_key = owner.summary(note).await.unwrap().key.unwrap();

    owner
        .grant(team, leaver.account(), Access::None)
        .await
        .unwrap();
    assert_eq!(editor.key_work().await.unwrap().rotations, [team]);
    editor.tend().await.unwrap();
    assert!(owner.key_work().await.unwrap().rotations.is_empty());
    let new_key = editor.summary(note).await.unwrap().key.unwrap();
    assert_ne!(new_key, old_key, "the editor's rotation kept the old key");

    editor
        .save(note, &TextBlock::of("after"), before)
        .await
        .unwrap();
    assert_eq!(text(&harness.second(&owner).await, note).await, "after");
    leaver.refresh_keys(Some(vec![note, team])).await.unwrap();
    assert!(!leaver.keys().has(new_key));

    harness.stop().await;
}
