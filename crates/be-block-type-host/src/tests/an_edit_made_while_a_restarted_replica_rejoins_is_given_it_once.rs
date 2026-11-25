use std::time::Instant;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::FromEditor;
use be_type_example_counter::{Counter, CounterContent};

use super::*;

/// Adds `by` to the open counter as its editor.
fn add(replicas: &mut Replicas<Quiet>, block: Uuid, editor: EditorId, by: i64) {
    replicas.relay_from_editor(
        block,
        editor,
        FromEditor::Operate {
            gesture: None,
            operation: CounterContent::encode_operation(&Counter::add(by)),
        }
        .encode(),
        Instant::now(),
    );
}

/// An editor adds 1, which a seal keeps, and 2, which the order holds past it. The type's
/// module stops, and while the replica opened again rejoins its session the editor adds 4. The
/// replica is opened on the server's journal once it rejoined and only then given its edits,
/// the 4 among them, which the replica it rejoined from never had, so it submits nothing the
/// opening on the server's journal is then given again: the counter reads 7, not 11.
#[test]
fn an_edit_made_while_a_restarted_replica_rejoins_is_given_it_once() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the counter is made");
        replicas
            .open((&peer, &shared), block, CounterContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        replicas.attach(block, editor, true);
        add(&mut replicas, block, editor, 1);
        while replicas
            .status(block)
            .is_none_or(|status| status.taken < 1 || !status.journaled)
        {
            step(&peer, &shared, &mut replicas).await;
        }
        let before = replicas.status(block).and_then(|status| status.head);
        replicas.seal_later(block, true);
        while replicas
            .status(block)
            .is_none_or(|status| status.head == before)
        {
            step(&peer, &shared, &mut replicas).await;
        }
        add(&mut replicas, block, editor, 2);
        while replicas
            .status(block)
            .is_none_or(|status| status.taken < 2 || !status.journaled)
        {
            step(&peer, &shared, &mut replicas).await;
        }

        replicas.modules.restart_type(CounterContent::CONTENT_TYPE);
        replicas.pump((&peer, &shared)).await;
        assert!(replicas.joining(), "the replica opened again rejoins");
        add(&mut replicas, block, editor, 4);
        // The module would take the edit, and submit it, before the join is answered.
        for _ in 0..3 {
            replicas.modules.tick_due();
            replicas.pump((&peer, &shared)).await;
        }
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        while replicas
            .status(block)
            .is_none_or(|status| !status.clean || status.taken < 1)
        {
            step(&peer, &shared, &mut replicas).await;
        }
        for _ in 0..3 {
            step(&peer, &shared, &mut replicas).await;
        }
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the counter is open");
        let count = CounterContent::decode(&bytes)
            .expect("the counter decodes")
            .root()
            .value();
        assert_eq!(count, 7, "each edit reaches the replica opened again once");
    });
}
