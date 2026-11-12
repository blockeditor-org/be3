use super::*;
use be_type_text::{TextBlock, TextContent};

#[test]
fn a_search_index_is_compacted_by_the_device_that_writes_it() {
    let harness = Harness::start();
    harness.connect();
    let device = Device::online(&harness);
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    let (root, index) = (Uuid::new_v4(), Uuid::new_v4());
    let words = [
        "apple", "banana", "cherry", "damson", "elder", "fig", "grape", "hazel", "iris",
        "juniper", "kiwi", "lemon",
    ];
    let notes: Vec<Uuid> = words.iter().map(|_| Uuid::new_v4()).collect();
    device.runtime.block_on(async {
        peer.create_with_id::<TextContent>(root, be_block_graph::BlockParent::Root)
            .await
            .expect("the root is made");
        crate::search::make_index(&peer, root, index)
            .await
            .expect("the index is made");
        for (n, note) in notes.iter().enumerate() {
            peer.create_with_id::<TextContent>(*note, be_block_graph::BlockParent::Block(root))
                .await
                .expect("a note is made");
            let body = words[n].to_owned();
            let saved = peer
                .save(*note, &TextBlock::of(&body), None)
                .await
                .expect("the note saves");
            let head = saved.published().expect("the note is published");
            let commit = peer.load_commit(head).await.expect("its commit loads");
            let batch = be_lib_search::Batch {
                put: vec![be_lib_search::Put {
                    doc: be_lib_search::DocId {
                        block: note.as_bytes().to_vec(),
                        key: String::new(),
                    },
                    commit: be_lib_search::Commit {
                        time: commit.time,
                        id: *head.hash().as_bytes(),
                    },
                    source: None,
                    text: Some(be_lib_search::Text {
                        name: "Note".to_owned(),
                        body,
                    }),
                }],
                ..be_lib_search::Batch::default()
            };
            assert_eq!(
                crate::search::write(peer.as_ref(), index, batch).await,
                Ok(true),
                "batch {n} is written"
            );
            peer.drain(|_| false)
                .await
                .expect("the batch is published");
        }
        let head = crate::search::head_of(peer.as_ref(), index)
            .await
            .expect("the index has a head");
        let content = crate::search::load(peer.as_ref(), head)
            .await
            .expect("the index loads");
        assert!(
            content.index().segment_count() <= 8,
            "twelve batches were compacted, leaving {} segments",
            content.index().segment_count()
        );
        for (n, note) in notes.iter().enumerate() {
            let query = be_lib_search::Query::parse(words[n]);
            let hits = crate::search::search_index(peer.as_ref(), index, &query, 10)
                .await
                .expect("the index is searched");
            assert_eq!(
                hits.iter()
                    .map(|hit| Uuid::from_slice(&hit.doc.block).expect("a block id"))
                    .collect::<Vec<_>>(),
                vec![*note],
                "{} is found after compaction", words[n]
            );
        }
    });
}
