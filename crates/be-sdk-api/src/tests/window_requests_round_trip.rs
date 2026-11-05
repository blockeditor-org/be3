use super::*;

#[test]
fn window_requests_round_trip() {
    {
        let message = LayoutCommit {
            windows: vec![ScreenWindow {
                screen: ScreenId(7),
                window: WindowRequests {
                    cursor: CursorIcon::Crosshair,
                    pointer_locked: true,
                    ime: None,
                    handles_back: true,
                    wants_keyboard: true,
                    intercepted_keys: vec![KeyChord {
                        key: Key::Escape,
                        modifiers: Modifiers::default(),
                        tap: true,
                    }],
                    pastes: vec!["image/png".to_owned(), "text/plain".to_owned()],
                    clipboard_keys: true,
                    keyboard_toggles: 3,
                    every_sample: true,
                },
            }],
            ..frame(1)
        };
        round_trips(&message);
    }
}
