use std::time::Instant;

use be_block_content::LiveEdit;
use be_sdk_type_guest::editor::FromEditor;
use be_sdk_type_testing::sections::SetSection;

use super::*;

/// A part a module's writer finished stays pinned in its replica's reach, handed there as
/// `write_part` hands it (`Reach::hand_written`), until a commit that reuses it is sealed. An
/// edit sets a section to the part, the replica publishes content reusing it once the edit
/// goes quiet, and the publish lets the pin go (`Reach::named`). A collection then keeps the
/// part, which the commit names, and no longer notes it. Nothing else names the part
/// meanwhile: no hold of it waits in the outbox.
#[test]
fn a_part_a_module_publishes_is_let_go_of_by_its_reach_and_kept_by_the_commit() {
    let clock = be_block_client::Clock::manual();
    with_test_types_on(clock.clone(), |peer, shared, mut replicas| async move {
        let block = block(&peer).await;
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let opened = replicas.status(block).expect("the block is open").head;

        let bytes: Vec<u8> = (0..40_000u32)
            .map(|at| (at.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        let mut writer = peer.part_writer(block).await.expect("a writer");
        writer.write(&bytes).expect("written");
        let written = writer.finish().expect("sealed");
        let part = written.part().expect("a byte was written");
        let objects = peer.part_objects(&[part]).expect("the part is here");
        let vault = peer.commits().vault();
        assert_eq!(vault.unnamed_count(), objects.len());
        let reach = replicas.reach(block).expect("the block is open");
        reach.lock().unwrap().hand_written(part, written);

        let editor = 7;
        replicas.attach(block, editor, true);
        replicas.pump((&peer, &shared)).await;
        let named = SetSection {
            index: 1,
            bytes: Vec::new(),
            part: Some(part),
        };
        replicas.relay_from_editor(
            block,
            editor,
            FromEditor::Operate {
                gesture: None,
                operation: Sections::encode_operation(&named),
            }
            .encode(),
            Instant::now(),
        );
        while replicas.status(block).is_none_or(|status| status.clean) {
            step(&peer, &shared, &mut replicas).await;
        }
        assert_eq!(reach.lock().unwrap().pinned(), 1, "only the edit names it");
        // Its module seals once the edit goes quiet.
        while replicas
            .status(block)
            .is_none_or(|status| !status.clean || status.head == opened)
        {
            if let Some(at) = replicas.modules.wake_at() {
                clock.advance(at.saturating_sub(clock.now()));
            }
            step(&peer, &shared, &mut replicas).await;
        }
        assert_eq!(reach.lock().unwrap().pinned(), 0, "the publish let the pin go");

        assert_eq!(peer.collect_unnamed(&Default::default()).unwrap(), 0);
        assert_eq!(vault.unnamed_count(), 0, "found named, so no longer noted");
        for object in &objects {
            assert!(
                be_block_store::ObjectStore::has(vault.store(), *object).unwrap(),
                "{object}, named by the commit, stays"
            );
        }
        let head = replicas
            .status(block)
            .and_then(|status| status.head)
            .expect("sealed");
        let manifest = peer.load_reached(head).await.expect("the head is read").manifest;
        assert!(
            vault.objects(&manifest).unwrap().contains(&part.hash()),
            "the head reuses the part"
        );
    });
}
