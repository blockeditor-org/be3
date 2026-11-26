use be_block_content::Part;
use be_block_graph::Access;
use be_block_store::ObjectStore;
use be_type_text::TextContent;

use super::*;

/// Two accounts on one device share its store but not their refs, so what one account's part
/// writers let go of another's commits may name. Sealing is convergent under the block's one
/// key: a part the member streams of the same bytes is the owner's let go of, and a whole write
/// of the same bytes puts the same objects. The member's offline commit names both, and the
/// owner's collection, which walks only the owner's commits, keeps them, even after a restart
/// that came while the member's part was still held: the member's claims reached the ledger
/// as they were made, not once the part was let go of.
#[tokio::test]
async fn what_another_scope_on_the_device_names_survives_a_collection() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let member = harness.member(&owner, "member@example.com").await;
    let folder = workspace(&owner, "Parts").await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let public = owner.verified_identity(member.account()).unwrap();
    owner
        .share(folder, member.account(), Access::Edit, &public)
        .await
        .unwrap();

    let file = RefsFile::new();
    let refs = file.open();
    let store = MemoryStore::new();
    let mine = harness.device(&owner, &refs, &store).await;
    let theirs = harness.device(&member, &refs, &store).await;
    let head = theirs.newest_head(block).await.unwrap();
    let streamed = pseudorandom(16 * 1024, 21);
    let whole = pseudorandom(16 * 1024, 22);
    let mut let_go = Vec::new();
    for bytes in [&streamed, &whole] {
        let mut writer = mine.part_writer(block).await.unwrap();
        writer.write(bytes).unwrap();
        let written = writer.finish().unwrap();
        let_go.extend(mine.part_objects(&[written.part().unwrap()]).unwrap());
    }
    assert_eq!(mine.commits().vault().unnamed_count(), let_go.len());
    assert_eq!(
        theirs.commits().vault().unnamed_count(),
        0,
        "noted as the owner's"
    );
    drop(theirs);

    let offline = harness.offline(&member, &refs, &store);
    let mut writer = offline.part_writer(block).await.unwrap();
    writer.write(&streamed).unwrap();
    let written = writer.finish().unwrap();
    let part = written.part().expect("bytes were written");
    let saved = offline
        .write(
            block,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                Part::node(vec![Part::Reused(part), Part::from(whole.as_slice())]),
                head,
            ),
        )
        .await
        .unwrap();
    assert!(matches!(saved, Saved::Kept(_)), "{saved:?}");
    // The host stops before the part is let go of, and starts again.
    drop(offline);
    drop(mine);
    drop(refs);
    let refs = file.open();
    let mine = harness.offline(&owner, &refs, &store);
    let offline = harness.offline(&member, &refs, &store);
    drop(written);

    assert_eq!(
        mine.collect_unnamed(&Default::default()).unwrap(),
        0,
        "the member's commit names them"
    );
    assert_eq!(offline.collect_unnamed(&Default::default()).unwrap(), 0);
    for object in &let_go {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }
    assert_eq!(
        offline
            .commits()
            .vault()
            .read_part(part, 0, part.length())
            .unwrap(),
        streamed
    );

    harness.stop().await;
}
