use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::tuning::{BLEND_FROM, BLEND_TO, SHOWN_FOR, STEP, Up};

const M: u32 = 50;
const UP: u32 = 103;
const LEFT: u32 = 105;

fn tune(space: &mut Space, now: Instant, key_pressed: Key, evdev: u32) -> Vec<Incoming> {
    send(
        space,
        now,
        [
            Input::Modifiers(Modifiers {
                alt: true,
                ..Modifiers::CTRL
            }),
            key(key_pressed, evdev, true),
            key(key_pressed, evdev, false),
            Input::Modifiers(Modifiers::NONE),
        ],
    )
}

#[test]
fn ctrl_alt_keys_tune_the_mouse_s_up_and_show_it_by_the_cursor() {
    let (mut space, now) = space();
    assert_eq!(space.tuning.simulator, Up::Sky);
    send(&mut space, now, [Input::PointerMoved(centre())]);

    let delivered = tune(&mut space, now, Key::M, M);
    assert_eq!(
        space.tuning.simulator,
        Up::View,
        "M moves on to the view's up"
    );
    assert_eq!(
        space.tuning.headset,
        Up::Sky,
        "and leaves the headset's alone"
    );
    assert_eq!(
        keys(&delivered),
        0,
        "the app sees neither the press nor the release"
    );

    tune(&mut space, now, Key::M, M);
    assert_eq!(
        space.tuning.simulator,
        Up::Blend {
            from: BLEND_FROM,
            to: BLEND_TO
        }
    );
    tune(&mut space, now, Key::ArrowLeft, LEFT);
    tune(&mut space, now, Key::ArrowUp, UP);
    let Up::Blend { from, to } = space.tuning.simulator else {
        panic!("still a blend: {:?}", space.tuning.simulator);
    };
    assert!(
        (from - (BLEND_FROM - STEP)).abs() < 1e-5,
        "left starts the blend sooner"
    );
    assert!((to - (BLEND_TO + STEP)).abs() < 1e-5, "up ends it later");

    assert_eq!(
        space.tuning.label(Some(now)).as_deref(),
        Some(
            "Simulator mouse: up turns from the sky to the top of the view between 35 and 75 degrees"
        )
    );
    assert_eq!(
        space.tuning.label(Some(now + SHOWN_FOR)),
        None,
        "the label goes away"
    );

    let (mut space, now, context) = painted_space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    tune(&mut space, now, Key::M, M);
    let windows = [window(&context, WindowId::MAIN, PLATFORM, "Main")];
    assert_scene(
        &mut space,
        now,
        &windows,
        "ctrl_alt_keys_tune_the_mouse_s_up_and_show_it_by_the_cursor",
    );
}
