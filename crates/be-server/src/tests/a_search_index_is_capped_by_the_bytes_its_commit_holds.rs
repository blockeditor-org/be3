use super::*;
use be_block_graph::SEARCH_INDEX_TYPE;

/// A search index's commit may hold at most `max_index_bytes`: its base's objects, less what it
/// drops, plus what it adds, each counted once, however many blocks also hold it.
#[tokio::test]
async fn a_search_index_is_capped_by_the_bytes_its_commit_holds() {
    let measure = Author::new(Uuid::nil());
    let shared_text = b"text a document and its index both hold\n".repeat(8);
    let (probe, probe_chunks) = measure.compose(&shared_text, 0, None);
    let bytes_of = |author: &Author, hashes: &[Hash]| -> u64 {
        hashes
            .iter()
            .map(|hash| author.store.get(*hash).unwrap().unwrap().len() as u64)
            .sum()
    };
    // The cap fits the first index commit exactly: its commit object and the chunks it shares
    // with the document, counted once.
    let cap = bytes_of(&measure, &measure.objects_of(probe, &probe_chunks));

    let harness = Harness::configured(ServerConfig {
        max_index_bytes: cap,
        ..ServerConfig::OPEN
    })
    .await;
    let mut client = harness.client().await;
    let account = client.register("index@example.com").await;
    client.organization("index").await;
    let author = Author::new(account);
    let root = client.create_block(BlockParent::Root).await;
    let document = client.create_block(BlockParent::Block(root)).await;
    let index = Uuid::new_v4();
    let key = Uuid::new_v4();
    client
        .create_typed(
            index,
            SEARCH_INDEX_TYPE,
            BlockParent::Block(root),
            key,
            KeySet {
                keys: vec![row(key, index, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        )
        .await;
    let now = be_block_commit::now_milliseconds();

    let (held, chunks) = author.compose(&shared_text, now, None);
    let published = client
        .publish(&author, document, held, chunks.clone(), None, now)
        .await;
    assert!(published.is_ok(), "{published:?}");

    // Composed at the probe's time, so the commit object is the probe's size.
    let (first, index_chunks) = author.compose(&shared_text, 0, None);
    assert_eq!(
        index_chunks, chunks,
        "the index holds the document's chunks"
    );
    assert_eq!(
        bytes_of(&author, &author.objects_of(first, &index_chunks)),
        cap
    );
    let published = client
        .publish(&author, index, first, index_chunks, None, now)
        .await;
    assert!(published.is_ok(), "{published:?}");

    // The base plus a large addition passes the cap.
    let mut large = shared_text.clone();
    large.extend((0..64 * 1024u32).map(|at| (at.wrapping_mul(2_654_435_761) >> 13) as u8));
    let (grown, grown_chunks) = author.compose(&large, now, Some(first));
    let refused = client
        .publish(&author, index, grown, grown_chunks, Some(first), now)
        .await;
    assert!(refused_as(&refused, ErrorCode::OverQuota), "{refused:?}");

    // What replaces the base's chunks with fewer bytes fits: the dropped ones no longer count.
    let (smaller, smaller_chunks) = author.compose(b"less\n", now, Some(first));
    assert!(bytes_of(&author, &author.objects_of(smaller, &smaller_chunks)) <= cap);
    let published = client
        .publish(&author, index, smaller, smaller_chunks, Some(first), now)
        .await;
    assert!(published.is_ok(), "{published:?}");
    harness.stop().await;
}
