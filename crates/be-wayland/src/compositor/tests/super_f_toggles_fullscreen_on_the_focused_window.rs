use super::*;

const KEY_F: u32 = 33;
const KEY_LEFTMETA: u32 = 125;

fn super_f(harness: &mut Harness) {
    harness.frame(vec![
        Event::PhysicalKey {
            code: KEY_LEFTMETA,
            pressed: true,
        },
        Event::PhysicalKey {
            code: KEY_F,
            pressed: true,
        },
        Event::PhysicalKey {
            code: KEY_F,
            pressed: false,
        },
        Event::PhysicalKey {
            code: KEY_LEFTMETA,
            pressed: false,
        },
    ]);
    harness.settle();
}

#[test]
fn super_f_toggles_fullscreen_on_the_focused_window() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));

    super_f(&mut harness);
    assert!(harness.client.received.fullscreen);
    assert_eq!(harness.window(id), Rect::from_min_size(Pos2::ZERO, SCREEN));
    assert_eq!(
        harness.client.received.keys,
        [(KEY_LEFTMETA, true), (KEY_LEFTMETA, false)],
        "the F that toggled it never reaches the window"
    );

    harness.acknowledge(&window);
    super_f(&mut harness);
    assert!(!harness.client.received.fullscreen);
    assert_eq!(harness.window(id).size(), SHOWN);
}
