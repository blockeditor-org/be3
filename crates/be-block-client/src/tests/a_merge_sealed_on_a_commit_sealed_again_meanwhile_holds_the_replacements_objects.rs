use be_type_text::{TextBlock, TextContent};

use super::*;

/// A merge is sealed on a commit of this device's that waits in the outbox (`ours`), and that
/// commit is sealed again (`Peer::reseal`, `Peer::restamp`) after the merge read it and before it
/// is queued: here under other content, as a reseal under a new epoch holds other objects. The
/// refs refuse the merge as it goes on a replaced commit, and `seal` makes it again on the
/// replacement: with the replacement's own objects among what it holds, which the server would
/// otherwise never be sent, and none of the old one's.
#[tokio::test]
async fn a_merge_sealed_on_a_commit_sealed_again_meanwhile_holds_the_replacements_objects() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let team = workspace(&laptop, "Team").await;
    let note = laptop
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let base = laptop
        .save(note, &TextBlock::of("one"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    assert!(
        offline
            .save(note, &TextBlock::of("two, from the laptop"), Some(base))
            .await
            .is_ok_and(|saved| matches!(saved, Saved::Kept(_)))
    );
    let ours = offline.local_head(note).unwrap();
    let phone = harness.second(&owner).await;
    let theirs = phone
        .save(note, &TextBlock::of("two, from the phone"), Some(base))
        .await
        .unwrap()
        .published()
        .unwrap();
    harness.reconnect(&offline, owner.token()).await;
    offline.fetch_history(theirs).await.unwrap();

    let paused = std::sync::Arc::new(tokio::sync::Notify::new());
    let resume = std::sync::Arc::new(tokio::sync::Notify::new());
    offline.pause_before_queue(paused.clone(), resume.clone());
    let encoded = TextBlock::of("two, merged").encode();
    let (merged, replaced) = tokio::join!(
        offline.seal(
            note,
            Write::encoded(TextContent::CONTENT_TYPE, &encoded, Some(theirs))
                .merging(Some(base), ours),
        ),
        async {
            paused.notified().await;
            let key = offline.sealing_key(note).await.unwrap();
            let commit = offline.commits().get(ours).unwrap();
            let replacement = offline
                .commits()
                .write(
                    key,
                    TextContent::CONTENT_TYPE,
                    TextBlock::of("two, from the laptop, sealed again").encode(),
                    commit.author,
                    commit.time,
                    Some(base),
                    Vec::new(),
                )
                .unwrap();
            let (sequence, pending) = offline.pending_publish(note, ours).unwrap();
            let chunks = offline
                .objects_of(&offline.commits().get(replacement).unwrap().manifest)
                .await
                .unwrap();
            let moved = std::collections::HashMap::from([(ours, replacement)]);
            let rewritten = vec![(
                sequence,
                ours,
                refs::Pending {
                    commit: replacement,
                    chunks,
                    ..pending
                },
            )];
            let replaced = offline
                .kept_refs()
                .replace_publishes(note, rewritten, &moved)
                .unwrap();
            resume.notify_one();
            replaced
        }
    );
    assert!(replaced, "ours was not replaced");
    let Sealed::Sealed(merged) = merged.unwrap() else {
        panic!("the merge was not sealed");
    };
    let again = offline.resealed_as(ours);
    assert!(again.is_empty(), "ours replaced nothing itself: {again:?}");
    let (_, replacement) = offline
        .pending_commits(note)
        .into_iter()
        .filter_map(|commit| offline.pending_publish(note, commit))
        .find(|(_, pending)| pending.commit != merged)
        .expect("the replacement of ours waits");
    assert_eq!(offline.resealed_as(replacement.commit), [ours]);

    let (_, merge) = offline.pending_publish(note, merged).unwrap();
    assert_eq!(merge.expected, Some(theirs));
    assert_eq!(merge.parents, [theirs, replacement.commit]);
    let held = offline
        .objects_of(&offline.commits().get(replacement.commit).unwrap().manifest)
        .await
        .unwrap();
    assert!(merge.chunks.contains(&replacement.commit.hash()));
    assert!(
        held.iter().all(|object| merge.chunks.contains(object)),
        "the merge leaves out the replacement's objects"
    );
    assert!(
        !merge.chunks.contains(&ours.hash()),
        "the merge still holds ours"
    );
    let old = offline
        .objects_of(&offline.commits().get(ours).unwrap().manifest)
        .await
        .unwrap();
    assert_ne!(old, held, "sealed again, ours holds other objects");
    assert!(
        old.iter()
            .filter(|object| !held.contains(object))
            .all(|object| !merge.chunks.contains(object)),
        "the merge holds objects only ours held"
    );

    harness.stop().await;
}
