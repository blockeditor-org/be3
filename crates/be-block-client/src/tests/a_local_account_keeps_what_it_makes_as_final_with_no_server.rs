use super::*;

use be_type_text::{TextBlock, TextContent};

fn local(
    account: Uuid,
    organization: Uuid,
    refs: &refs::RefsDb,
    store: &MemoryStore,
) -> Peer<MemoryStore> {
    Peer::local(
        PeerConfig::new("", identity("local@example.com"), Credentials::Header)
            .chunker(ChunkerConfig::SMALL)
            .refs(refs.clone()),
        account,
        organization,
        store.clone(),
    )
    .unwrap()
}

/// A local account's peer has no server (plans/local_accounts.md): a commit is the head once it
/// is sealed, what it makes is listed from what it holds, nothing waits to be sent, and all of
/// it is there when the device starts again.
#[tokio::test]
async fn a_local_account_keeps_what_it_makes_as_final_with_no_server() {
    let (account, organization) = (Uuid::now_v7(), Uuid::now_v7());
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let peer = local(account, organization, &refs, &store);
    assert!(peer.is_local());
    assert!(!peer.is_attached());

    let team = workspace(&peer, "Team").await;
    let note = peer
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let one = peer.save(note, &TextBlock::of("one"), None).await.unwrap();
    let Saved::Published(one) = one else {
        panic!("a local commit was not final: {one:?}");
    };
    let Ok(Saved::Published(two)) = peer.save(note, &TextBlock::of("two"), Some(one)).await else {
        panic!("a local commit was not final");
    };
    assert!(
        matches!(
            peer.save(note, &TextBlock::of("stale"), Some(one)).await,
            Ok(Saved::Rejected { head: Some(head) }) if head == two
        ),
        "a commit on a head the block left is refused, as a server refuses it"
    );
    assert_eq!(peer.local_head(note), Some(two));
    assert_eq!(peer.known_remote_head(note), Some(two));

    let folder = peer
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    peer.set_parent(folder, BlockParent::Root).await.unwrap();
    assert_eq!(peer.pending(), refs::PendingCounts::default());
    let roots: Vec<Uuid> = peer
        .children(BlockParent::Root)
        .await
        .unwrap()
        .iter()
        .map(|block| block.id)
        .collect();
    assert!(roots.contains(&team) && roots.contains(&folder));
    let inside: Vec<Uuid> = peer
        .children(BlockParent::Block(team))
        .await
        .unwrap()
        .iter()
        .map(|block| block.id)
        .collect();
    assert_eq!(inside, [note]);
    assert_eq!(peer.drain(|_| false).await.unwrap(), []);
    drop(peer);

    let again = local(account, organization, &refs, &store);
    assert_eq!(text(&again, note).await, "two");
    let history: Vec<be_block_commit::CommitId> = again
        .history(note)
        .await
        .unwrap()
        .iter()
        .map(|entry| entry.commit)
        .collect();
    assert_eq!(history, [one, two]);
    assert!(
        again.children(BlockParent::Root).await.unwrap().len() >= 2,
        "what was made is listed after a restart"
    );
}
