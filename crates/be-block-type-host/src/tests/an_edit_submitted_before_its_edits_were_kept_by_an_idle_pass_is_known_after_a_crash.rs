use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// Steps until `done` holds of this device's records of `block`.
async fn step_until(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    what: &str,
    done: impl Fn(&Peer<MemoryStore>) -> bool,
) {
    // A bound on a signal that comes, not a wait for time to pass: a dropped edit never comes.
    let waited = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while !done(peer) {
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

/// The host crashes after the server numbered two edits and before an idle pass kept anything
/// since: that they were submitted was written ahead of each submission, so opened again the
/// host gives them again under their numbers, which the order holds, and the next edit typed
/// takes the number after rather than one the order holds already, which would drop it.
#[test]
fn an_edit_submitted_before_its_edits_were_kept_by_an_idle_pass_is_known_after_a_crash() {
    let clock = be_block_client::Clock::manual();
    let modules_clock = clock.clone();
    with_test_types_on(clock, |peer, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        peer.save(block, &TextBlock::of(""), None)
            .await
            .expect("the empty text is published");
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let editor = 7;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        type_at_start(&mut replicas, block, editor, &mut mirror, "a");
        step_until(
            &peer,
            &shared,
            &mut replicas,
            "kept the first edit as held",
            |peer| {
                heard_entries(peer, block) >= 1
                    && peer
                        .open_edits(block)
                        .is_some_and(|(kept, _)| kept.given_up >= 1)
            },
        )
        .await;

        replicas.hold_kept_edits = true;
        receive_frames(&shared, editor, &mut mirror);
        for typed in ["b", "d"] {
            let before = heard_entries(&peer, block);
            type_at_start(&mut replicas, block, editor, &mut mirror, typed);
            step_until(
                &peer,
                &shared,
                &mut replicas,
                "heard the edit ordered",
                |peer| heard_entries(peer, block) > before,
            )
            .await;
            receive_frames(&shared, editor, &mut mirror);
        }
        drop(replicas);

        let mut replicas = Replicas::new(test_modules(modules_clock).await);
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let editor = 8;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        let before = heard_entries(&peer, block);
        type_at_start(&mut replicas, block, editor, &mut mirror, "c");
        step_until(
            &peer,
            &shared,
            &mut replicas,
            "heard the edit typed after ordered",
            |peer| heard_entries(peer, block) > before,
        )
        .await;
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the text is open");
        let text = TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"));
        assert_eq!(text, "cdba", "each edit landed once");
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
