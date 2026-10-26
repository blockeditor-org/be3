use super::*;

#[tokio::test]
async fn deletions_reach_the_other_side() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let checkout = harness.checkout("checkout");
    write(&checkout, "keep.md", "kept\n");
    write(&checkout, "drop.md", "dropped\n");
    write(&checkout, "gone/inside.md", "inside\n");
    sync(&peer, &checkout, code).await;
    let gone = child(&peer, code, "gone").await;
    let sketch = block(&peer, gone.id, "Sketch", &CanvasContent::default()).await;
    sync(&peer, &checkout, code).await;

    std::fs::remove_file(checkout.join("drop.md")).unwrap();
    std::fs::remove_dir_all(checkout.join("gone")).unwrap();
    let report = sync(&peer, &checkout, code).await;
    assert_eq!(report.deleted, 2, "{report}");
    let names: Vec<String> = children(&peer, code)
        .await
        .into_iter()
        .map(|listed| listed.name)
        .collect();
    assert_eq!(names, ["keep.md"]);
    let deleted = peer.summary(sketch).await.unwrap();
    assert_ne!(deleted.parent, Some(BlockParent::Block(code)));

    let keep = child(&peer, code, "keep.md").await;
    peer.delete(keep.id).await.unwrap();
    let report = sync(&peer, &checkout, code).await;
    assert_eq!(report.removed, 1, "{report}");
    assert!(!checkout.join("keep.md").exists());
    harness.stop().await;
}
