use super::*;

#[tokio::test]
async fn a_rename_on_disk_moves_the_same_block() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let checkout = harness.checkout("checkout");
    write(&checkout, "old/inner/note.md", "a note\n");
    write(&checkout, "old/plan.md", "a plan\n");
    sync(&peer, &checkout, code).await;
    let old = child(&peer, code, "old").await;
    let plan = child(&peer, old.id, "plan.md").await;
    let inner = child(&peer, old.id, "inner").await;
    let note = child(&peer, inner.id, "note.md").await;

    std::fs::rename(checkout.join("old"), checkout.join("new")).unwrap();
    std::fs::rename(checkout.join("new/plan.md"), checkout.join("plan.md")).unwrap();
    std::fs::rename(
        checkout.join("new/inner/note.md"),
        checkout.join("new/inner/renamed.md"),
    )
    .unwrap();
    let report = sync(&peer, &checkout, code).await;
    assert_eq!((report.created, report.deleted), (0, 0), "{report}");

    let new = child(&peer, code, "new").await;
    assert_eq!(new.id, old.id);
    assert_eq!(child(&peer, code, "plan.md").await.id, plan.id);
    let inner_now = child(&peer, new.id, "inner").await;
    assert_eq!(inner_now.id, inner.id);
    let renamed = child(&peer, inner.id, "renamed.md").await;
    assert_eq!(renamed.id, note.id);
    assert_eq!(text_of(&peer, renamed.id).await, "a note\n");

    let elsewhere = harness.checkout("elsewhere");
    sync(&peer, &elsewhere, code).await;
    peer.define(note.id, |metadata| {
        metadata.insert(&be_block::NAME, &"moved.md".to_owned());
    })
    .await
    .unwrap();
    relist(&peer, inner.id, be_block::ChildChange::Delete(note.id)).await;
    list_in(&peer, code, note.id).await;
    peer.set_parent(note.id, BlockParent::Block(code))
        .await
        .unwrap();
    let report = sync(&peer, &elsewhere, code).await;
    assert_eq!(report.moved, 1, "{report}");
    assert_eq!(read(&elsewhere, "moved.md"), "a note\n");
    assert!(!elsewhere.join("new/inner/renamed.md").exists());
    harness.stop().await;
}
