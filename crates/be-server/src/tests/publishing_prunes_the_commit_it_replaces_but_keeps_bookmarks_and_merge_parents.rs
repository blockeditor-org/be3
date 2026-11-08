use super::*;

use be_block_commit::retention::MINUTE;

#[tokio::test]
async fn publishing_prunes_the_commit_it_replaces_but_keeps_bookmarks_and_merge_parents() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("keeper@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;
    // Half an hour ago, where the policy keeps one state per minute: every commit below lands in
    // that one minute.
    let minute = (be_block_commit::now_milliseconds() / MINUTE - 30) * MINUTE;

    let (bookmark, bookmark_chunks) = author.compose(b"a bookmarked draft\n", minute, None);
    client
        .upload(
            &author.store,
            &author.objects_of(bookmark, &bookmark_chunks),
        )
        .await;
    let _ = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                bookmark,
                None,
                Vec::new(),
                objects_change(&[], bookmark_chunks.clone()),
                minute,
                true,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    let mut head = bookmark;
    let mut drafts = Vec::new();
    for (step, text) in [&b"ours\n"[..], b"theirs\n"].into_iter().enumerate() {
        let time = minute + 1_000 * (step as i64 + 1);
        let (draft, chunks) = author.compose(text, time, Some(head));
        let _ = client
            .publish(&author, block, draft, chunks, Some(head), time)
            .await;
        drafts.push(draft);
        head = draft;
    }
    let (ours, theirs) = (drafts[0], drafts[1]);
    assert!(
        !history(&mut client, block).await.contains(&ours),
        "a commit replaced in its minute was kept"
    );

    let (merge, merge_chunks) = author.compose(b"ours\ntheirs\n", minute + 3_000, Some(theirs));
    client
        .upload(&author.store, &author.objects_of(merge, &merge_chunks))
        .await;
    let response = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                merge,
                Some(theirs),
                vec![theirs, ours],
                objects_change(&author.held_by(Some(theirs)), merge_chunks.clone()),
                minute + 3_000,
                false,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        response.is_ok(),
        "the merge was not published: {response:?}"
    );
    let (after, after_chunks) =
        author.compose(b"ours\ntheirs\nafter\n", minute + 4_000, Some(merge));
    let _ = client
        .publish(
            &author,
            block,
            after,
            after_chunks,
            Some(merge),
            minute + 4_000,
        )
        .await;

    let kept = history(&mut client, block).await;
    assert!(kept.contains(&bookmark), "the bookmark was pruned");
    assert!(kept.contains(&theirs), "a merge's parent was pruned");
    assert!(kept.contains(&after), "the head was pruned");
    assert!(
        !kept.contains(&merge),
        "the commit the head replaced in its minute was kept"
    );
    let response = client.download(bookmark.hash()).await;
    assert!(
        matches!(response, Ok(Some(_))),
        "the bookmark's object was deleted"
    );
    let response = client.download(merge.hash()).await;
    assert!(
        matches!(response, Ok(None)),
        "the pruned commit's object survived"
    );

    harness.stop().await;
}

async fn history(client: &mut TestClient, block: Uuid) -> Vec<CommitId> {
    client
        .history(block)
        .await
        .into_iter()
        .map(|entry| entry.commit)
        .collect()
}
