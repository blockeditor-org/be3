use be_sdk_type_api::{Failure, embed::exports::replicas as module};

use super::*;
use crate::{OUTSIDE_REQUEST, Scope};

/// A reader outside a block's session replays its journal (`Modules::readable`), and the type
/// reads the parts the journal names under that request: only parts sealed under that block's
/// keys. A part of another block, which this device can read, is refused as outside the
/// request, nothing is read, and the reader reads the head alone.
#[test]
fn a_replay_reads_only_parts_of_the_block_whose_journal_it_was_handed() {
    with_test_types(async |peer, _, mut replicas| {
        let read_block = block(&peer).await;
        let own = section_parts(&peer, &written(&peer, read_block, &[]).await).await[0];
        let other = block(&peer).await;
        let foreign = section_parts(&peer, &written(&peer, other, &[]).await).await[0];
        let module = replicas
            .modules
            .module_of(Sections::CONTENT_TYPE)
            .expect("the sections have a module");
        let tag = replicas.modules.tag(module);
        let read = peer.current(read_block).await.expect("the block reads");
        let head = read.summary.head;
        let ask = |part| {
            let journal = journal_by(tag, head, part);
            move |connection: &mut be_sdk_type_api::ops::Connection| {
                module::replay(connection, Sections::CONTENT_TYPE, None, journal)
            }
        };

        let mut scope = Scope::of_block(read_block);
        let answer = replicas
            .modules
            .ask_reading(module, &*peer, &mut scope, ask(foreign))
            .await;
        match answer {
            Ok(Err(Failure::Malformed(error))) => assert!(
                error.contains(OUTSIDE_REQUEST),
                "the read of another block's part is refused: {error}"
            ),
            other => panic!("the replay cannot read its base: {other:?}"),
        }
        assert!(scope.read().is_empty(), "nothing was read: {scope:?}");

        let mut scope = Scope::of_block(read_block);
        let _ = replicas
            .modules
            .ask_reading(module, &*peer, &mut scope, ask(own))
            .await;
        assert_eq!(scope.read(), [own], "a part of its own block is read");

        let mut replayed = read.clone();
        replayed.journal = Some(journal_by(tag, head, foreign));
        assert_eq!(
            replicas.modules.readable(&*peer, replayed).await,
            read.stored.map(|stored| stored.bytes),
            "the reader reads the head alone"
        );
    });
}
