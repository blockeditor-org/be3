use be_block_content::BlockContent;
use be_sdk_type_testing::tally::{Tally, TallyOp};

use super::*;

/// Waits on the modules and what they answer, without hearing the connection: what the server
/// ordered meanwhile reaches the replicas only at the next `step`.
async fn pump_until(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    what: &str,
    done: impl Fn(&Replicas<Quiet>) -> bool,
) {
    let waited = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        replicas.pump((peer, shared)).await;
        while !done(replicas) {
            replicas.woken().await;
            replicas.pump((peer, shared)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "never {what}");
}

async fn step_until(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    what: &str,
    done: impl Fn(&Replicas<Quiet>) -> bool,
) {
    let waited = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while !done(replicas) {
            step(peer, shared, replicas).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "never {what}");
}

/// How many entries the server's order holds of `block`, as this device heard them.
fn heard_entries(peer: &Peer<MemoryStore>, block: Uuid) -> usize {
    peer.local_journal(block)
        .ok()
        .flatten()
        .map_or(0, |journal| journal.entries.len())
}

/// The last local edit the submissions `block`'s replica keeps hold.
fn last_kept(replicas: &Replicas<Quiet>, block: Uuid) -> u64 {
    replicas
        .submissions_of(block)
        .and_then(|(_, _, kept)| kept.into_iter().max())
        .unwrap_or_default()
}

fn operate(
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
    editor: EditorId,
    mirror: &mut be_sdk_type_guest::editor::EditorMirror<Tally>,
    operation: TallyOp,
) {
    let message = mirror.operate(operation).expect("the editor has content");
    replicas.relay_from_editor(block, editor, message.encode(), std::time::Instant::now());
}

/// An edit is on its way when the order restarts (the holder sealed a session state past
/// `RESTART_STATE_BYTES`): the server numbers it after the restart, every replica passes it over,
/// and its replica submits it again carried over. The host keeps that submission's identity,
/// though the replica's status naming the restart comes in the same pass, so a replica opened
/// after a crash would give the edit again under it; and the edit lands once.
#[test]
fn an_edit_carried_over_a_restart_keeps_the_identity_it_goes_again_under_across_a_crash() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<Tally>(BlockParent::Root)
            .await
            .expect("the tally is made");
        peer.save(block, &Tally::default(), None)
            .await
            .expect("the empty tally is published");
        replicas
            .open((&peer, &shared), block, Tally::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let editor = 7;
        let mut mirror = attach_mirror::<Tally>(&peer, &shared, &mut replicas, block, editor).await;
        // Only what is written ahead of a submission is kept: no idle pass writes over it.
        replicas.hold_kept_edits = true;

        let grow = be_sdk_type_guest::RESTART_STATE_BYTES + 1;
        operate(&mut replicas, block, editor, &mut mirror, TallyOp::Grow(grow));
        replicas.pump((&peer, &shared)).await;
        step_until(&peer, &shared, &mut replicas, "had the growth ordered", |replicas| {
            heard_entries(&peer, block) >= 1
                && replicas
                    .submissions_of(block)
                    .is_some_and(|(submitting, ..)| submitting == 0)
        })
        .await;
        receive_frames(&shared, editor, &mut mirror);

        // The seal restarts the order; the edit goes before the replica hears the restart.
        replicas.seal((&peer, &shared), block, false).await;
        pump_until(&peer, &shared, &mut replicas, "submitted the restart", |replicas| {
            replicas
                .submissions_of(block)
                .is_some_and(|(submitting, ..)| submitting == 1)
        })
        .await;
        let grown = last_kept(&replicas, block);
        operate(&mut replicas, block, editor, &mut mirror, TallyOp::Add(10));
        pump_until(&peer, &shared, &mut replicas, "submitted the edit", |replicas| {
            last_kept(replicas, block) > grown
        })
        .await;
        let added = last_kept(&replicas, block);
        step_until(&peer, &shared, &mut replicas, "heard the restart", |replicas| {
            replicas
                .submissions_of(block)
                .is_some_and(|(_, restarts, _)| restarts >= 1)
        })
        .await;
        let (_, _, kept) = replicas
            .submissions_of(block)
            .expect("the block is open");
        assert_eq!(
            kept,
            [added],
            "the edit's submission carried over the restart is kept, and none from before it"
        );

        drop(replicas);
        let mut replicas = Replicas::new(test_modules(be_block_client::Clock::real()).await);
        replicas
            .open((&peer, &shared), block, Tally::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        // A replica reports no restarts until it loads: that drops none of what it was given,
        // before it submits them again.
        pump_until(&peer, &shared, &mut replicas, "heard from the reopened replica", |replicas| {
            replicas
                .submissions_of(block)
                .is_some_and(|(_, restarts, _)| restarts == 0)
        })
        .await;
        let (_, _, kept) = replicas
            .submissions_of(block)
            .expect("the block is open again");
        assert_eq!(kept, [added], "the reopened replica's first status kept them");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        step_until(&peer, &shared, &mut replicas, "settled the edit", |replicas| {
            replicas
                .submissions_of(block)
                .is_some_and(|(submitting, ..)| submitting == 0)
        })
        .await;
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the tally is open");
        let tally = Tally::decode(&bytes).expect("the tally decodes");
        assert_eq!(tally.count, 10, "the edit landed once");
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
