use super::*;
use be_block_content::{ChildChange, SEARCH_INDEX, be_block_model::BlockRef};
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// What a search for `text` answers once every queued edit is indexed.
fn found(text: &str) -> Vec<Uuid> {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(|shared| {
        let answer = shared.search.answer(&ask)?;
        let mut found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        found.sort();
        answer.complete.then_some(found)
    })
}

fn note(workspace: Uuid, text: &str) -> Uuid {
    let block = Uuid::new_v4();
    create(
        block,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block_content::BlockMetadata::named("Note"),
        Some(TextBlock::of("").encode()),
        false,
    );
    assert_eq!(
        change_child(
            workspace,
            WorkspaceContent::CONTENT_TYPE,
            ChildChange::Add(block)
        ),
        Some(true)
    );
    open(block, TextContent::CONTENT_TYPE);
    type_text(block, 0, text);
    flush();
    block
}

/// Waits until search is idle and everything it wrote is sent.
fn settled() {
    crate::wait_until(|shared| {
        (shared.waiting == 0
            && shared
                .search
                .busy_with(crate::search::wants_index(shared))
                .is_none())
        .then_some(())
    });
}

#[test]
fn an_index_over_the_merge_cap_is_compacted_while_idle_and_answers_the_same() {
    let harness = Harness::start();
    harness.connect();
    // Every merge a batch's compaction could make is over this cap, and no idle pass runs yet.
    crate::with_shared_mut(|shared| {
        shared.search.merge_bytes = Some(1);
        shared.search.hold_idle = true;
    });
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Team"),
        None,
        false,
    );
    wait_until("knew the workspace's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let words = ["apples", "bananas", "cherries", "damsons", "elderberries"];
    let notes: Vec<Uuid> = words
        .iter()
        .map(|word| note(workspace, &format!("{word} and figs")))
        .collect();
    let mut all = notes.clone();
    all.sort();
    for (word, note) in words.iter().zip(&notes) {
        assert_eq!(found(word), [*note]);
    }
    assert_eq!(found("figs"), all);
    settled();
    let index = crate::wait_until(|shared| {
        shared
            .graph
            .get(workspace)?
            .metadata
            .get(&SEARCH_INDEX)
            .map(|BlockRef(index)| index)
    });
    let peer = crate::with_shared(|shared| shared.peer.clone())
        .flatten()
        .expect("the host has a peer");
    let runtime = runtime();
    let before = runtime.block_on(crate::search::segment_count(&peer, index));
    assert!(
        before >= notes.len(),
        "no batch's compaction merged over the cap: {before} segments"
    );

    // Idle, the host merges what the cap left, a commit at a time, until nothing is over it.
    crate::with_shared_mut(|shared| shared.search.hold_idle = false);
    send(Command::Sync);
    crate::wait_until(|shared| {
        let head = peer.local_head(index);
        (shared.search.idle_compactions > 0
            && shared.search.compacted_at(index) == Some(head)
            && shared.waiting == 0)
            .then_some(())
    });
    settled();
    let after = runtime.block_on(crate::search::segment_count(&peer, index));
    assert!(
        after < before,
        "the idle pass merged segments: {before} before, {after} after"
    );
    // Asked afresh, so each search reads the compacted index rather than answering from before.
    for (word, note) in words.iter().zip(&notes) {
        assert_eq!(found(&format!("{word} ")), [*note], "{word} is found as before");
    }
    assert_eq!(found("figs "), all);
}
