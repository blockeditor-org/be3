use super::*;
use be_sdk_beui::beui::Input;
use be_sdk_test::Relayed;

/// What a frame of a held resize costs the session: one `Held` a frame, appending the frame's
/// edit alone, smaller than the presence the same frame shows (about 70 bytes to about 125),
/// and one landing at the peer. A frame of input is a frame of the clock, so held edits relay
/// no faster than presence does, and need no throttle of their own.
#[test]
fn a_held_resize_relays_one_small_message_a_frame() {
    let mut target = card();
    target.style.stroke_width = 0.0;
    let canvas = vector([target.clone()]);
    let mut mover = open(&canvas, false, &[]);
    let mut watcher = open(&canvas, false, &[]);
    join_session(&mut mover, &mut watcher);
    mover.presence_visible(true);
    mover.run();

    let shape = format!("infinite-canvas.entity.{}", target.id);
    let before = mover.rect_of(&shape);
    let scale = before.width() / 184.0;
    mover.click(&shape);
    let from = before.center() + Vec2::new(86.0, 56.0) * scale;
    mover.down(from, PointerButton::Primary);
    relay_session(&mut mover, &mut watcher, FIRST_PEER);
    mover.take_shown_presence();
    watcher.store().take_landed(None);
    for step in 1..=30 {
        mover.step(vec![Input::PointerMoved(
            from + Vec2::new(step as f32 * 3.0, step as f32 * 2.0),
        )]);
        let relayed = mover.store().take_relayed(None);
        let held: Vec<usize> = relayed
            .iter()
            .filter_map(|relayed| match relayed {
                Relayed::Message(bytes) => {
                    let edits = relayed.held_edits()?;
                    assert_eq!(edits, 1, "frame {step} appends its edit alone");
                    Some(bytes.len())
                }
                Relayed::Submission { .. } => panic!("frame {step} submits nothing"),
            })
            .collect();
        let presence: usize = mover
            .take_shown_presence()
            .iter()
            .filter_map(|shown| shown.value.as_ref().map(Vec::len))
            .sum();
        watcher
            .store()
            .receive_relayed(None, FIRST_PEER, PEER_ORIGINS + FIRST_PEER, relayed);
        let landed = watcher.store().take_landed(None);
        watcher.step(Vec::new());
        // The first move only passes the drag's threshold.
        if step == 1 {
            assert!(held.is_empty());
            continue;
        }
        assert_eq!(held.len(), 1, "frame {step} relays one Held");
        assert!(
            held[0] < presence,
            "frame {step}: its Held is {} bytes, its presence {presence}",
            held[0]
        );
        assert_eq!(landed, Some(1), "frame {step} is one landing at the peer");
    }
}
