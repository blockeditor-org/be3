use std::collections::HashSet;

use be_block::{BlobOp, LiveEdit, Part, PartRef, Stored};

use super::*;

const ENTRY: usize = 8 * 1024;
const GROUP: usize = 16;

/// Fixed-size entries saved in groups of `GROUP`, each group a part with a part per entry.
struct Log {
    entries: Vec<Vec<u8>>,
    saved: Vec<Option<PartRef>>,
}

impl Log {
    fn edit(&mut self, index: usize, entry: Vec<u8>) {
        self.entries[index] = entry;
        self.saved[index / GROUP] = None;
    }
}

impl BlockContent for Log {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6c6f_67);

    fn encode(&self) -> Vec<u8> {
        self.entries.concat()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let entries: Vec<Vec<u8>> = bytes.chunks(ENTRY).map(<[u8]>::to_vec).collect();
        Ok(Self {
            saved: vec![None; entries.len().div_ceil(GROUP)],
            entries,
        })
    }

    fn parts(&self) -> Part<'_> {
        Part::Node(
            self.entries
                .chunks(GROUP)
                .zip(&self.saved)
                .map(|(group, saved)| match saved {
                    Some(saved) => Part::Reused(*saved),
                    None => Part::Node(group.iter().map(Part::from).collect()),
                })
                .collect(),
        )
    }

    fn from_stored(stored: &Stored) -> Result<Self, ContentError> {
        let mut log = Self::decode(&stored.bytes)?;
        if let Some(root) = &stored.root
            && root.children.len() == log.saved.len()
        {
            log.saved = root
                .children
                .iter()
                .map(|group| Some(group.reference))
                .collect();
        }
        Ok(log)
    }
}

#[tokio::test]
async fn a_large_block_saved_after_a_small_change_uploads_only_what_changed() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let email = "archivist@example.com";
    let author = Peer::connect(
        PeerConfig::new(
            &relay.url(),
            identity(email),
            Credentials::Token(harness.server.register(email, email).await),
        ),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let workspace = workspace(&author, "Archive").await;

    let block = author
        .create::<Log>(BlockParent::Block(workspace))
        .await
        .unwrap();
    let log = Log::decode(&pseudorandom(512 * ENTRY, 5)).unwrap();
    let before = relay.heard();
    let first = author.save(block, &log, None).await.unwrap();
    let first = first.published().expect("the first save is published");
    assert!(relay.heard() - before > 4 * 1024 * 1024);

    let mut log = author.open::<Log>(block).await.unwrap().unwrap();
    log.edit(300, pseudorandom(ENTRY, 6));
    let before = relay.heard();
    let second = author.save(block, &log, Some(first)).await.unwrap();
    let second = second.published().expect("the edit is published");
    let sent = relay.heard() - before;

    let vault = author.commits().vault();
    let old: HashSet<_> = vault
        .objects(&author.commits().get(first).unwrap().manifest)
        .unwrap()
        .into_iter()
        .collect();
    let new = vault
        .objects_besides(&author.commits().get(second).unwrap().manifest, &old)
        .unwrap();
    assert_eq!(
        new.len(),
        3,
        "an edit to one entry saves its chunk, its group's node and the root"
    );
    // The publish still names every object of the commit; sending only what changed is
    // plans/large_blocks.md, 3.
    assert!(
        sent < 48 * 1024,
        "an 8 KiB edit to a 4 MiB block sent {sent} bytes"
    );
    let reader = harness.second(&author).await;
    let read = reader.open::<Log>(block).await.unwrap().unwrap();
    assert_eq!(read.entries, log.entries);

    let image_block = author
        .create::<ImageContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    let saved = author
        .save(image_block, &image("photo.png", 2 * 1024 * 1024, 7), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let mut renamed = author
        .open::<ImageContent>(image_block)
        .await
        .unwrap()
        .unwrap();
    let mut header = renamed.header().clone();
    header.source_name = "renamed.png".into();
    renamed.apply(&BlobOp::SetHeader(header));
    let before = relay.heard();
    author
        .save(image_block, &renamed, Some(saved))
        .await
        .unwrap()
        .published()
        .expect("the rename is published");
    let sent = relay.heard() - before;
    assert!(
        sent < 8 * 1024,
        "renaming a 2 MiB image sent {sent} bytes"
    );
    let read = reader
        .open::<ImageContent>(image_block)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.header().source_name, "renamed.png");
    assert_eq!(read.data(), renamed.data());

    harness.stop().await;
}
