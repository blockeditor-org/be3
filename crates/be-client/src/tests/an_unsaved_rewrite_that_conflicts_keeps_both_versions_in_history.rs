use super::*;

use be_block::{TextContent, TextOp};

#[tokio::test]
async fn an_unsaved_rewrite_that_conflicts_keeps_both_versions_in_history() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    let base: String = (0..20).map(|line| format!("line {line}\n")).collect();
    let shared = phone
        .save(block, &TextContent::from(base.as_str()), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let laptop = harness.shared(&phone).await;

    let mut on_phone = Live::<_, TextContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let retyped: String = (0..20).map(|line| format!("rewritten {line}\n")).collect();
    on_phone
        .edit(TextOp::delete(0, base.len() as u64))
        .await
        .unwrap();
    on_phone.edit(TextOp::insert(0, &retyped)).await.unwrap();

    let theirs_text = base.replace("line 7\n", "line seven, edited\n");
    let theirs = laptop
        .save(
            block,
            &TextContent::from(theirs_text.as_str()),
            Some(shared),
        )
        .await
        .unwrap()
        .published()
        .unwrap();

    let outcome = on_phone.reconcile().await.unwrap();
    assert!(
        !outcome.is_clean(),
        "a full rewrite absorbed a concurrent edit"
    );
    let conflicts = on_phone.take_conflicts();
    assert_eq!(conflicts.len(), 1, "the conflict was not recorded");
    assert_eq!(conflicts[0].theirs, theirs);

    let ours = conflicts[0].ours;
    laptop.fetch_history(ours).await.unwrap();
    assert_eq!(
        laptop
            .open_commit::<TextContent>(ours)
            .await
            .unwrap()
            .text(),
        retyped,
        "the unsaved side of the conflict was not kept"
    );
    let head = on_phone.head().expect("the merge was published");
    let merge = laptop.load_commit(head).await.unwrap();
    assert!(merge.parents.contains(&theirs));
    assert!(merge.parents.contains(&ours));

    harness.stop().await;
}
