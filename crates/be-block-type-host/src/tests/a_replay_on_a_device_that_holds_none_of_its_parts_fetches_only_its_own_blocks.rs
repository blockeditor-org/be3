use be_sdk_type_api::{Failure, embed::exports::replicas as module};

use be_block_store::ObjectStore;

use super::*;
use crate::{OUTSIDE_REQUEST, Scope};

/// A device that holds none of the parts a replay names finds whose each is without fetching
/// another block's: the server answers a part's object only as the replayed block's
/// (`Peer::is_part_of`), so a part of another block this device could read is refused and
/// never fetched, and one of the block's own is fetched and read.
#[test]
fn a_replay_on_a_device_that_holds_none_of_its_parts_fetches_only_its_own_blocks() {
    with_another_device(|peer, phone, _, mut replicas| async move {
        let mut parts = Vec::new();
        for _ in 0..2 {
            let made = block(&peer).await;
            let saved = peer.save(made, &base(), None).await;
            assert!(
                matches!(saved, Ok(be_block_client::Saved::Published(_))),
                "the block publishes: {saved:?}"
            );
            parts.push((
                made,
                section_parts(&peer, &written(&peer, made, &[]).await).await[0],
            ));
        }
        let [(read_block, own), (_, foreign)] = parts[..] else {
            unreachable!("two blocks were made")
        };
        // The other device knows the block and its keys, and holds none of its content.
        let summary = phone.summary(read_block).await.expect("the block is known");
        phone
            .refresh_keys(Some(vec![read_block]))
            .await
            .expect("the keys are read");
        let held = |part: be_block_store::PartRef| {
            phone
                .commits()
                .vault()
                .store()
                .has(part.hash())
                .expect("the store answers")
        };
        assert!(
            !held(own) && !held(foreign),
            "the other device holds neither part"
        );
        let module = replicas
            .modules
            .module_of(Sections::CONTENT_TYPE)
            .expect("the sections have a module");
        let tag = replicas.modules.tag(module);
        let head = summary.head;
        let ask = |part| {
            let journal = journal_by(tag, head, part);
            move |connection: &mut be_sdk_type_api::ops::Connection| {
                module::replay(connection, Sections::CONTENT_TYPE, None, journal)
            }
        };

        let mut scope = Scope::of_block(read_block);
        let answer = replicas
            .modules
            .ask_reading(module, &phone, &mut scope, ask(foreign))
            .await;
        match answer {
            Ok(Err(Failure::Malformed(error))) => assert!(
                error.contains(OUTSIDE_REQUEST),
                "the read of another block's part is refused: {error}"
            ),
            other => panic!("the replay cannot read its base: {other:?}"),
        }
        assert!(scope.read().is_empty(), "nothing was read: {scope:?}");
        assert!(!held(foreign), "another block's part is not fetched");

        let mut scope = Scope::of_block(read_block);
        let _ = replicas
            .modules
            .ask_reading(module, &phone, &mut scope, ask(own))
            .await;
        assert_eq!(scope.read(), [own], "a part of its own block is read");
        assert!(held(own), "its own part is fetched");
    });
}
