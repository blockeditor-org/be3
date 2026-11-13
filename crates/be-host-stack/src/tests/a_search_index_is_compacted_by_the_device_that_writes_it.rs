use super::*;

/// A batch putting one document, made in this test rather than read from a block: compaction
/// reads only what the index holds.
fn one(word: &str, time: i64, watermark: Option<u64>) -> be_lib_search::Batch {
    be_lib_search::Batch {
        put: vec![be_lib_search::Put {
            doc: be_lib_search::DocId {
                block: Uuid::new_v4().as_bytes().to_vec(),
                key: String::new(),
            },
            commit: be_lib_search::Commit {
                time,
                id: [u8::try_from(time % 251).unwrap_or(0); 32],
            },
            source: None,
            text: Some(be_lib_search::Text {
                name: "Note".to_owned(),
                body: word.to_owned(),
            }),
        }],
        watermark,
        ..be_lib_search::Batch::default()
    }
}

/// Words no other is a prefix of, as the index matches prefixes.
fn word(n: usize) -> String {
    format!("q{}z", n * 7 + 1000)
}

/// Makes a root with an index on `device`, for its batches.
fn indexed(device: &Device) -> Uuid {
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    let (root, index) = (Uuid::new_v4(), Uuid::new_v4());
    device.runtime.block_on(async {
        peer.create_with_id::<be_type_text::TextContent>(root, be_block_graph::BlockParent::Root)
            .await
            .expect("the root is made");
        crate::search::make_index(&peer, root, index)
            .await
            .expect("the index is made");
        peer.drain(|_| false).await.expect("the index is published");
    });
    index
}

/// Writes batches `range` on `device`, each published before the next; a catch-up's
/// (`watermark`) moves the watermark to ten times its number.
fn write_all(device: &Device, index: Uuid, range: std::ops::Range<usize>, catch_up: bool) {
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        for n in range {
            let watermark = catch_up.then_some(n as u64 * 10);
            assert_eq!(
                crate::search::write(peer.as_ref(), index, one(&word(n), n as i64 + 1, watermark))
                    .await,
                Ok(true),
                "batch {n} is written"
            );
            peer.drain(|_| false).await.expect("the batch is published");
        }
    });
}

fn index_of(device: &Device, index: Uuid) -> be_block::search_index::SearchIndexContent {
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        let head = crate::search::head_of(peer.as_ref(), index)
            .await
            .expect("the index has a head");
        crate::search::load(peer.as_ref(), head)
            .await
            .expect("the index loads")
    })
}

fn finds(device: &Device, index: Uuid, n: usize) -> usize {
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        crate::search::search_index(
            peer.as_ref(),
            index,
            &be_lib_search::Query::parse(&word(n)),
            10,
        )
        .await
        .expect("the index is searched")
        .len()
    })
}

#[test]
fn a_search_index_is_compacted_by_the_device_that_writes_it() {
    let harness = Harness::start();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 0..15, false);
    let content = index_of(&device, index);
    // Tiered by fours: fifteen documents are three segments of four and three of one.
    assert_eq!(content.index().segment_count(), 6, "the batches were compacted");
    for n in 0..15 {
        assert_eq!(finds(&device, index, n), 1, "{} is found", word(n));
    }
}

#[test]
fn a_catch_up_forgets_what_the_index_retired_before_the_states_the_server_keeps() {
    let harness = Harness::start();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 1..17, true);
    assert!(
        index_of(&device, index).index().remembered() > 0,
        "the last compaction's retired segments are remembered while a kept state predates it"
    );
    // The server keeps one state a slot (be-server's `retention`): the earlier ones in it are
    // gone, so a catch-up's watermark passes every compaction before it.
    write_all(&device, index, 17..19, true);
    assert_eq!(
        index_of(&device, index).index().remembered(),
        0,
        "what was retired before every kept state is forgotten"
    );
}

#[test]
fn an_offline_catch_up_forgets_nothing() {
    let mut harness = Harness::start();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 1..5, true);
    let remembered = index_of(&device, index).index().remembered();
    assert!(remembered > 0, "the compaction retired segments");

    harness.stop_server();
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        assert_eq!(
            crate::search::write(peer.as_ref(), index, one(&word(5), 6, Some(50))).await,
            Ok(true),
            "the batch is written offline"
        );
    });
    assert_eq!(
        index_of(&device, index).index().remembered(),
        remembered,
        "offline, nothing is forgotten"
    );
}

#[test]
fn an_edits_batch_compacts_without_asking_the_server() {
    let harness = Harness::relayed();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 1..16, true);
    let histories = harness
        .relay()
        .count(|method| method == be_block_protocol::__args::BLOCKS_LIST_HISTORY);
    let before = *histories.borrow();
    harness.relay().stall();
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        // The sixteenth fills a tier, so the batch compacts.
        assert_eq!(
            crate::search::write(peer.as_ref(), index, one(&word(16), 17, None)).await,
            Ok(true),
            "the batch is written while the server is silent"
        );
    });
    assert_eq!(*histories.borrow(), before, "an edit's batch asked for no history");
    assert!(
        index_of(&device, index).index().segment_count() < 6,
        "the batch compacted"
    );
}
