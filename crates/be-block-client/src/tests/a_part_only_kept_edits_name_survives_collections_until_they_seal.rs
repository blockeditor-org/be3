use be_block_content::Part;
use be_block_store::ObjectStore;
use be_type_text::TextContent;

use crate::{KeptEdit, OpenEdits};

use super::*;

/// A module's part, held by the server and named only by an edit kept on the device (no commit
/// names it yet), survives collections across a restart, offline, while the edit is kept and
/// once it is set aside; the edit then seals a commit that reuses the part, which reads back
/// whole. Collected, the part would be lost: offline, no server holds it to fetch again.
#[tokio::test]
async fn a_part_only_kept_edits_name_survives_collections_until_they_seal() {
    let harness = Harness::start().await;
    let owner = harness.owner("writer@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let folder = workspace(&laptop, "Parts").await;
    let block = laptop
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let head = laptop.newest_head(block).await.unwrap();
    let bytes = pseudorandom(16 * 1024, 11);
    let mut writer = laptop.part_writer(block).await.unwrap();
    writer.write(&bytes).unwrap();
    let written = writer.finish().unwrap();
    let part = written.part().expect("bytes were written");
    laptop.hold_part(block, part).await.unwrap();
    let objects = laptop.part_objects(&[part]).unwrap();
    // The module's edit that names the part, kept before the host stops.
    let kept = OpenEdits {
        origin: 7,
        ..OpenEdits::default()
    };
    let edits = vec![KeptEdit::Frame {
        editor: 1,
        bytes: b"an edit naming the part".to_vec(),
    }];
    laptop
        .keep_open_edits(block, kept, edits, Vec::new())
        .await
        .unwrap();
    drop(written);
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let vault = offline.commits().vault();
    assert_eq!(vault.unnamed_count(), objects.len(), "still noted after the restart");
    assert_eq!(offline.collect_unnamed().unwrap(), 0, "kept edits may name them");
    offline.set_aside_open_edits(block).unwrap();
    assert_eq!(offline.collect_unnamed().unwrap(), 0, "set aside, they may yet");
    for object in &objects {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }

    let saved = offline
        .write(
            block,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                Part::node(vec![Part::from(b"a header".as_slice()), Part::Reused(part)]),
                head,
            ),
        )
        .await
        .unwrap();
    assert!(matches!(saved, Saved::Kept(_)), "{saved:?}");
    assert_eq!(offline.collect_unnamed().unwrap(), 0);
    assert_eq!(vault.read_part(part, 0, part.length()).unwrap(), bytes);

    harness.stop().await;
}
