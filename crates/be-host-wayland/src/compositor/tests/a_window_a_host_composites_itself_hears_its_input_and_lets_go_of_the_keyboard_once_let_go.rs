use super::*;

const KEY_A: u32 = 30;

#[test]
fn a_window_a_host_composites_itself_hears_its_input_and_lets_go_of_the_keyboard_once_let_go() {
    // A host that composites a window itself (be-host's `RootShell`) places it with no view and
    // hands it what its own router delivered, as the window's view would catch it.
    let mut store = None;
    let mut harness = Harness::showing(None, |windows| {
        store = Some(windows);
        view! {
            <Frame />
        }
    });
    let windows = store.expect("the document was built");
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let place = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(300.0, 200.0));
    windows.place(id, Some(place));
    harness.settle();

    // A press in it, which gave it the keyboard, then a key.
    let now = harness.context().now();
    let mut catch = beui::Catch::default();
    let pressed = catch.caught(
        [
            (beui::Input::PointerMoved(Pos2::new(50.0, 40.0)), now),
            (beui::Input::Focus(true), now),
            (
                beui::Input::PointerButton {
                    pos: Pos2::new(50.0, 40.0),
                    button: beui::PointerButton::Primary,
                    pressed: true,
                },
                now,
            ),
            (
                beui::Input::Key(beui::Keystroke {
                    key: Some(beui::Key::A),
                    pressed: true,
                    repeat: false,
                    evdev: Some(KEY_A),
                    text: Some("a".to_owned()),
                }),
                now,
            ),
        ],
        beui::WindowId::MAIN,
        place.size(),
    );
    windows.deliver(id, pressed);
    harness.settle();
    assert!(
        harness.client.received.keyboard_entered,
        "the focus its host's router gave it gives it the keyboard"
    );
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true)],
        "the key it was handed reaches it"
    );

    // Let go of while it has the keyboard (its tab hidden), which its host's router says
    // nothing of: it lets go of the keyboard, as a view going does.
    windows.place(id, None);
    harness.settle();
    assert!(
        !harness.client.received.keyboard_entered,
        "a window let go of while it has the keyboard loses it"
    );
}
