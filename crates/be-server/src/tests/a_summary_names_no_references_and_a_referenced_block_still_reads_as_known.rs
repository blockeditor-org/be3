use be_block_graph::Access;

use super::*;

/// What a block references is in its content, which a reader decrypts: no summary names it, so
/// one referencing many blocks is no larger than one referencing none. The server still keeps
/// the references, so a block the reader may not view reads as known to exist through one it
/// views that references it.
#[tokio::test]
async fn a_summary_names_no_references_and_a_referenced_block_still_reads_as_known() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.personal().await;
    let author = Author::new(owner.id());
    let mut viewer = harness.member("viewer@example.com").await;
    let viewer_id = viewer.id();
    viewer.personal().await;
    let link = owner.link(None).await;
    let _ = viewer.accept(link, &[]).await;

    let page = owner.create_block(BlockParent::Root).await;
    let plain = owner.create_block(BlockParent::Root).await;
    let hidden = owner.create_block(BlockParent::Root).await;
    let mut linked = vec![hidden];
    for _ in 0..16 {
        linked.push(owner.create_block(BlockParent::Root).await);
    }
    for block in [page, plain] {
        let granted = owner.grant(block, viewer_id, Access::View).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }
    let before = viewer.try_read_block(hidden).await;
    assert!(
        before
            .as_ref()
            .is_ok_and(|summary| !summary.access.can_know_exists())
            || before.is_err(),
        "nothing yet lets the viewer know of the hidden block: {before:?}"
    );

    for (block, references, time) in [(page, linked.clone(), 1_000), (plain, Vec::new(), 2_000)] {
        let (commit, chunks) = author.compose(b"same length", time, None);
        owner
            .upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        let response = owner
            .call(|peer| {
                peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                    block,
                    expected: None,
                    commit: Some(be_block_protocol::NewCommit {
                        id: commit,
                        parents: Vec::new(),
                        held: objects_change(&[], chunks),
                        time,
                        pinned: false,
                        objects: Vec::new(),
                    }),
                    references: listing(&[], references),
                    metadata: Vec::new(),
                    journal: None,
                })
            })
            .await;
        assert!(
            matches!(
                response,
                Ok(be_block_protocol::PublishOutcome::Published(_))
            ),
            "{response:?}"
        );
    }
    assert_eq!(harness.kept_references(organization, page).await, linked);

    // Alike but for what changes with when they were written.
    let bytes = |summary: BlockSummary| {
        be_block_protocol::encoded_len(&BlockSummary {
            changed: 0,
            version: None,
            size: 0,
            tree_size: 0,
            ..summary
        })
    };
    let (page, plain) = (viewer.read_block(page).await, viewer.read_block(plain).await);
    assert!(page.access.can_view() && plain.access.can_view());
    assert_eq!(
        bytes(page),
        bytes(plain),
        "a summary of a block referencing seventeen others carries more than one referencing none"
    );

    let known = viewer.read_block(hidden).await;
    assert_eq!(
        (known.access, known.head, known.metadata.is_empty()),
        (Access::KnowExists, None, true),
        "a block the viewer may only know exists, through the page it views, reads as that"
    );

    harness.stop().await;
}
