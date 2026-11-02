use super::*;

#[tokio::test]
async fn references_declared_by_content_reach_the_graph() {
    let harness = Harness::start().await;
    let author = harness.owner("librarian@example.com").await;
    let first = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    let second = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    let index = author.create::<Link>(BlockParent::Root).await.unwrap();

    let head = author
        .save(
            index,
            &Link {
                targets: vec![first, second],
            },
            None,
        )
        .await
        .unwrap()
        .published()
        .unwrap();

    let backrefs = author.backrefs(first).await.unwrap();
    assert_eq!(backrefs.len(), 1);
    assert_eq!(backrefs[0].id, index);
    assert_eq!(author.backrefs(second).await.unwrap().len(), 1);

    let head = author
        .save(
            index,
            &Link {
                targets: vec![second],
            },
            Some(head),
        )
        .await
        .unwrap()
        .published()
        .unwrap();
    assert!(
        author.backrefs(first).await.unwrap().is_empty(),
        "dropping a reference left the edge behind"
    );
    assert_eq!(author.backrefs(second).await.unwrap().len(), 1);

    let commit = author.load_commit(head).await.unwrap();
    assert_eq!(
        commit.references,
        vec![second],
        "the commit did not record the edges of the state it sealed"
    );

    harness.stop().await;
}
