use super::*;

/// A text whose session state grew past the limit the test lowered its block's to
/// (`RESTART_LIMIT`, a fragment for each byte left between deleted ones) is sealed, which
/// restarts its order, while "a" is on its way and "b", typed after it, waits behind it. The restart reloads
/// the text with new positions, so the replica carries both over: "b" names where "a" was typed,
/// which the restarted text no longer holds. The process stops as a crash would before either
/// is numbered after the restart, keeping only what it wrote ahead of a submission. Opened
/// again, the host gives both edits again as typed, with the carried form of each, so both
/// land after it.
#[test]
fn a_text_edit_waiting_behind_one_in_flight_over_a_restart_lands_after_a_crash() {
    use be_type_text::{TextBlock, TextContent};

    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        peer.save(block, &TextBlock::of(""), None)
            .await
            .expect("the empty text is published");
        replicas.restart_limits.lower(block, RESTART_LIMIT);
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let editor = 7;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        // Only what is written ahead of a submission is kept: no idle pass writes over it.
        replicas.hold_kept_edits = true;
        let type_at = |replicas: &mut Replicas<Quiet>,
                       mirror: &mut be_sdk_type_guest::editor::EditorMirror<TextContent>,
                       at: usize,
                       text: &[u8]| {
            let shown = mirror.content().expect("the editor was shown the text");
            let typed = TextBlock::insert(shown, 3, at, text).expect("there is somewhere to type");
            let message = mirror.operate(None, typed).expect("the editor has content");
            replicas.relay_from_editor(block, editor, message.encode(), std::time::Instant::now());
        };

        // Session state past what a seal restarts the order at: every other byte of a long run
        // deleted leaves a fragment for each.
        let typed = 2_000;
        let kept = typed / 2;
        type_at(&mut replicas, &mut mirror, 0, &vec![b'x'; typed]);
        replicas.pump((&peer, &shared)).await;
        receive_frames(&shared, &mut replicas, block, editor, &mut mirror);
        let shown = mirror.content().expect("the editor was shown the text");
        let delete = |at: usize| {
            TextBlock::delete(shown, 2 * at + 1..2 * at + 2).expect("there is a byte to delete")
        };
        // In edits of a few hundred deletes each.
        let mut deletes = Vec::new();
        for from in (0..kept).step_by(250) {
            let mut deleted = delete(from);
            for at in from + 1..(from + 250).min(kept) {
                deleted.0.extend(delete(at).0);
            }
            deletes.push(deleted);
        }
        let mut fragmented = shown.clone();
        for deleted in &deletes {
            fragmented.apply(deleted);
        }
        let state = fragmented.session_state().len();
        assert!(
            state > RESTART_LIMIT as usize,
            "the deletes leave {state} bytes of session state"
        );
        for deleted in deletes {
            let message = mirror
                .operate(None, deleted)
                .expect("the editor has content");
            replicas.relay_from_editor(block, editor, message.encode(), std::time::Instant::now());
        }
        replicas.pump((&peer, &shared)).await;
        step_until_replicas(
            &peer,
            &shared,
            &mut replicas,
            "had the fragments ordered",
            |replicas, _| {
                replicas
                    .status(block)
                    .is_some_and(|status| status.journaled)
                    && replicas
                        .submissions_of(block)
                        .is_some_and(|(submitting, ..)| submitting == 0)
            },
        )
        .await;
        receive_frames(&shared, &mut replicas, block, editor, &mut mirror);

        // The seal restarts the order; "a" goes before the replica hears the restart, and "b"
        // waits behind it.
        replicas.seal((&peer, &shared), block, false).await;
        pump_until(
            &peer,
            &shared,
            &mut replicas,
            "submitted the restart",
            |replicas| {
                replicas
                    .submissions_of(block)
                    .is_some_and(|(submitting, ..)| submitting == 1)
            },
        )
        .await;
        let before = last_kept(&replicas, block);
        type_at(&mut replicas, &mut mirror, 0, b"a");
        pump_until(&peer, &shared, &mut replicas, "submitted a", |replicas| {
            last_kept(replicas, block) > before
        })
        .await;
        type_at(&mut replicas, &mut mirror, 1, b"b");
        replicas.pump((&peer, &shared)).await;
        step_until_replicas(
            &peer,
            &shared,
            &mut replicas,
            "heard the restart and submitted a carried over",
            |replicas, _| {
                replicas
                    .submissions_of(block)
                    .is_some_and(|(submitting, restarts, kept)| {
                        restarts >= 1 && submitting >= 1 && !kept.is_empty()
                    })
            },
        )
        .await;

        // A crash: only what the device wrote survives.
        drop(replicas);
        let mut replicas = Replicas::new(test_modules(be_block_client::Clock::real()).await);
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        step_until_replicas(
            &peer,
            &shared,
            &mut replicas,
            "settled both edits",
            |replicas, _| {
                replicas
                    .status(block)
                    .is_some_and(|status| status.journaled)
                    && replicas
                        .submissions_of(block)
                        .is_some_and(|(submitting, ..)| submitting == 0)
            },
        )
        .await;
        let text = text_of(&peer, &shared, &mut replicas, block).await;
        assert!(
            text.starts_with("abx") && text.len() == kept + 2,
            "both edits landed, once each: {:?}",
            &text[..text.len().min(8)]
        );
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
