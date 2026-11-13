use super::*;
use be_block_protocol::PublishOutcome;

/// Publishes `commit` onto `expected`, declaring `parents`.
async fn publish_onto(
    client: &mut TestClient,
    block: Uuid,
    commit: CommitId,
    expected: Option<CommitId>,
    parents: Vec<CommitId>,
    (held, chunks): (&[Hash], Vec<Hash>),
    time: i64,
) -> Result<PublishOutcome, Error> {
    let change = objects_change(held, chunks);
    client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                commit,
                expected,
                parents,
                change,
                time,
                false,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await
}

#[tokio::test]
async fn a_head_moves_only_to_a_commit_that_descends_from_it() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("one@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let (stray, stray_chunks) = author.compose(b"stray\n", 500, None);
    let (base, base_chunks) = author.compose(b"base\n", 1_000, None);
    for (commit, chunks) in [(stray, &stray_chunks), (base, &base_chunks)] {
        client
            .upload(&author.store, &author.objects_of(commit, chunks))
            .await;
    }
    // A merge onto a block with no commit yet carries the commit the server refused before it.
    let response = publish_onto(
        &mut client,
        block,
        base,
        None,
        vec![stray],
        (&[], base_chunks.clone()),
        1_000,
    )
    .await;
    assert!(
        matches!(response, Ok(PublishOutcome::Published(_))),
        "a publish onto no head that carries a parent was refused: {response:?}"
    );
    assert_eq!(client.read_block(block).await.head, Some(base));

    let (ours, ours_chunks) = author.compose(b"ours\n", 2_000, Some(base));
    client
        .upload(&author.store, &author.objects_of(ours, &ours_chunks))
        .await;
    for parents in [Vec::new(), vec![stray]] {
        let response = publish_onto(
            &mut client,
            block,
            ours,
            Some(base),
            parents.clone(),
            (&base_chunks, ours_chunks.clone()),
            2_000,
        )
        .await;
        assert!(
            refused_as(&response, ErrorCode::InvalidRequest),
            "a commit onto the head that left it out of {parents:?} was taken: {response:?}"
        );
        assert_eq!(client.read_block(block).await.head, Some(base));
    }

    // `ours` never reached the server, as a commit it refused and a merge carries.
    let (merge, merge_chunks) = author.compose(b"merged\n", 3_000, Some(base));
    client
        .upload(&author.store, &author.objects_of(merge, &merge_chunks))
        .await;
    let response = publish_onto(
        &mut client,
        block,
        merge,
        Some(base),
        vec![base, ours],
        (&base_chunks, merge_chunks),
        3_000,
    )
    .await;
    assert!(
        matches!(response, Ok(PublishOutcome::Published(_))),
        "a merge whose other parent the server never held did not move the head: {response:?}"
    );
    assert_eq!(client.read_block(block).await.head, Some(merge));

    harness.stop().await;
}
