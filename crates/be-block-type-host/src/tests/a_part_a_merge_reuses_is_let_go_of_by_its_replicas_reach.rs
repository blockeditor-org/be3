use be_block_client::{Drained, Saved};
use be_block_content::LiveEdit;
use be_sdk_type_testing::sections::SetSection;

use super::*;

/// A part finished by a writer of the block's open replica stays pinned in the replica's reach
/// (`Reach::hand_written`) until a commit that reuses it is sealed here. An edit sealed beside
/// the server's head sets a section to the part, another device moves the head on meanwhile,
/// and the merge `merge_diverged` writes reuses the part: it names the part as a publish of the
/// replica's would, so the pin goes (`Reach::named`).
#[test]
fn a_part_a_merge_reuses_is_let_go_of_by_its_replicas_reach() {
    with_another_device(|peer, phone, shared, mut replicas| async move {
        let block = block(&peer).await;
        let head = match peer.save(block, &base(), None).await {
            Ok(Saved::Published(head)) => head,
            saved => panic!("the base publishes: {saved:?}"),
        };
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }

        let mut writer = peer.part_writer(block).await.expect("a writer");
        writer
            .write(b"a section written as a part")
            .expect("written");
        let written = writer.finish().expect("sealed");
        let part = written.part().expect("a byte was written");
        let reach = replicas.reach(block).expect("the block is open");
        reach.lock().unwrap().hand_written(part, written);
        assert_eq!(
            reach.lock().unwrap().pinned(),
            1,
            "only the replica holds it"
        );

        let mut theirs = base();
        theirs.set(5, "FIVE");
        assert!(
            matches!(
                phone.save(block, &theirs, Some(head)).await,
                Ok(Saved::Published(_))
            ),
            "the other device moves the head on"
        );
        let mut ours = base();
        ours.apply(&SetSection {
            index: 2,
            bytes: Vec::new(),
            part: Some(part),
        });
        let edited = peer
            .write_content(block, Sections::CONTENT_TYPE, ours.parts())
            .await
            .expect("the edit is written");
        let edit = match peer
            .seal(block, be_block_client::Write::written(edited, Some(head)))
            .await
            .expect("the edit is sealed")
        {
            be_block_client::Sealed::Sealed(edit) => edit,
            sealed => panic!("the edit changes the block: {sealed:?}"),
        };
        let _ = peer.publish_sealed(block, edit).await;

        let mut merged = false;
        for _ in 0..4 {
            if peer.pending_commits(block).is_empty() {
                break;
            }
            for outcome in peer.drain(|_| false).await.expect("the outbox drains") {
                if let Drained::Diverged { remote, .. } = outcome {
                    replicas
                        .merge_diverged((&peer, &shared), block, remote)
                        .await
                        .expect("the block merges");
                    merged = true;
                }
            }
        }
        assert!(merged, "the edit diverged from the other device's head");
        assert_eq!(
            reach.lock().unwrap().pinned(),
            0,
            "the merge let the pin go"
        );
        let head = peer
            .remote_head(block)
            .await
            .expect("the head is read")
            .expect("the block has a head");
        let manifest = peer
            .load_reached(head)
            .await
            .expect("the head is read")
            .manifest;
        assert!(
            section_parts(&peer, &manifest).await.contains(&part),
            "the merge reuses the part"
        );
    });
}
