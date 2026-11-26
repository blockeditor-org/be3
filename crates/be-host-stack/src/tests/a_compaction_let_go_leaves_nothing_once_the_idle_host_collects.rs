use super::*;
use be_block_content::{ChildChange, SEARCH_INDEX, be_block_model::BlockRef};
use be_block_store::ObjectStore;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

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
    assert!(made(change_child(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        ChildChange::Add(block)
    )));
    open(block, TextContent::CONTENT_TYPE);
    type_text(block, 0, text);
    flush();
    block
}

/// Waits until a search for `text` answers once every queued edit is indexed.
fn found(text: &str) -> usize {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(&format!("answered a search for {text:?}"), |shared| {
        let answer = shared.search.answer(&ask)?;
        answer.complete.then_some(answer.hits.len())
    })
}

#[test]
fn a_compaction_let_go_leaves_nothing_once_the_idle_host_collects() {
    let harness = Harness::start();
    harness.connect();
    // Each batch leaves its segment unmerged and no idle pass merges them, so there is
    // something for the compaction below to merge. The collection runs all the same.
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
    let words = [
        "apples", "bananas", "cherries", "damsons", "elderberries", "figs", "grapes", "hazelnuts",
    ];
    for word in words {
        note(workspace, &format!("{word} and kiwis"));
        assert_eq!(found(word), 1);
    }
    let index = crate::wait_until("named the workspace's search index", |shared| {
        shared
            .graph
            .get(workspace)?
            .metadata
            .get(&SEARCH_INDEX)
            .map(|BlockRef(index)| index)
    });
    crate::wait_until("sent what search wrote", |shared| {
        (shared.waiting == 0
            && shared
                .search
                .busy_with(crate::search::wants_index(shared))
                .is_none())
        .then_some(())
    });
    let peer = crate::with_shared(|shared| shared.peer.clone())
        .flatten()
        .expect("the host has a peer");
    let runtime = runtime();

    // A compaction seals its merged parts into the vault, and is let go of before any commit
    // names them, as one whose head moved is.
    let vault = peer.commits().vault();
    let held: std::collections::HashSet<_> =
        vault.store().hashes().unwrap().into_iter().collect();
    let sealed = runtime.block_on(crate::search::compact_and_let_go(&peer, index));
    let head = peer.local_head(index).expect("the index has a head here");
    let manifest = peer.commits().get(head).expect("the head is held").manifest;
    let named: std::collections::HashSet<_> = vault
        .objects(&manifest)
        .expect("the head's objects are held")
        .into_iter()
        .collect();
    let unnamed: Vec<_> = sealed
        .iter()
        .filter(|object| !named.contains(object) && !held.contains(object))
        .copied()
        .collect();
    assert!(!unnamed.is_empty(), "the merge sealed parts nothing held names");
    // What the store held already is named by a commit of the index's history, so it stays.
    let shared: Vec<_> = sealed.iter().filter(|object| held.contains(object)).collect();

    // The host, idle, collects them; the head keeps every part.
    send(Command::Sync);
    flush();
    crate::wait_until("collected the merge's parts", |shared| {
        (shared.search.collected >= unnamed.len()).then_some(())
    });
    for object in &unnamed {
        assert!(
            !vault.store().has(*object).unwrap(),
            "{object}, which nothing names, was collected"
        );
    }
    for object in shared {
        assert!(vault.store().has(*object).unwrap(), "{object}, held before, stays");
    }
    assert_eq!(vault.unnamed_count(), 0, "nothing is left noted");
    assert!(vault.holds_all(&manifest).unwrap(), "the head is whole");
    let read = vault.read(&manifest).expect("every part of the head reads back");
    assert_eq!(read.len() as u64, manifest.length());
    for word in words {
        assert_eq!(found(&format!("{word} ")), 1, "{word} is found as before");
    }
}
