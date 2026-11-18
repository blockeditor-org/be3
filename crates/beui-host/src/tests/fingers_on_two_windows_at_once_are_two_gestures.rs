use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Window, WindowHandle, WindowId};

/// Fingers on two touch displays at once are each their own window's: one finger tapped in
/// each together is two one-finger taps, not a two-finger tap, while two fingers in one
/// window still are.
#[test]
fn fingers_on_two_windows_at_once_are_two_gestures() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let taps = create_signal::<Vec<usize>>(Vec::new());
    let document = build_node(move || {
        on_finger_tap(move |fingers: usize| {
            taps.update(|taps| taps.push(fingers));
            true
        });
        view! {
            <List spacing=0.0>
                <Frame width=300.0 height=300.0 />
                <Window id={window} size={vec2(300.0, 300.0)}>|_: WindowHandle|
                    <Frame />
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let touch = |finger: u64, phase: TouchPhase, pos: Pos2| Input::Touch {
        id: TouchId { device: 1, finger },
        phase,
        pos,
        force: None,
    };
    let (main, there) = (pos2(50.0, 50.0), pos2(120.0, 50.0));

    harness.frame_in(WindowId::MAIN, vec![touch(1, TouchPhase::Start, main)]);
    harness.frame_in(window, vec![touch(2, TouchPhase::Start, there)]);
    harness.frame_in(window, vec![touch(2, TouchPhase::End, there)]);
    harness.frame_in(WindowId::MAIN, vec![touch(1, TouchPhase::End, main)]);
    assert!(
        taps.with_untracked(|taps| taps.is_empty()),
        "a finger in each of two windows was taken for a two-finger tap"
    );

    harness.frame_in(window, vec![touch(3, TouchPhase::Start, main)]);
    harness.frame_in(window, vec![touch(4, TouchPhase::Start, there)]);
    harness.frame_in(window, vec![touch(4, TouchPhase::End, there)]);
    harness.frame_in(window, vec![touch(3, TouchPhase::End, main)]);
    assert_eq!(
        taps.get_untracked(),
        vec![2],
        "two fingers tapped in one window are a two-finger tap"
    );
}
