use super::*;

#[test]
fn frame_screens_and_reports_round_trip() {
    let mut request = region_screen(EditorRegion::Frame, 1, 2, 640, 480, 1.0);
    request.frame = Some(FrameSpec {
        chrome: FrameChrome::Drawn,
        content: Some(ChildRect {
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 200.0,
        }),
        top_bar: TopBar::Phone,
    });
    let screens = Message::Screens(ScreenSet {
        request_id: 3,
        screens: vec![request],
    });
    assert_eq!(
        decode_frame(&encode_frame(&screens).unwrap()).unwrap(),
        screens
    );

    let frames = Message::Frames(vec![FrameReport {
        screen: ScreenId(1),
        content: ChildRect {
            x: 0.0,
            y: 40.0,
            width: 640.0,
            height: 440.0,
        },
        painted: vec![ChildRect {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 40.0,
        }],
        floating: vec![ChildRect {
            x: 8.0,
            y: 44.0,
            width: 120.0,
            height: 90.0,
        }],
        claims: vec![PressClaim {
            modifiers: Modifiers {
                logo: true,
                ..Modifiers::default()
            },
            rect: ChildRect {
                x: 0.0,
                y: 40.0,
                width: 320.0,
                height: 440.0,
            },
        }],
        handles_back: true,
        wants_keyboard: true,
        intercepted_keys: vec![
            KeyChord {
                key: Key::Tab,
                modifiers: Modifiers {
                    alt: true,
                    ..Modifiers::default()
                },
                tap: false,
            },
            KeyChord {
                key: Key::Logo,
                modifiers: Modifiers::default(),
                tap: true,
            },
        ],
        description: Some(Description {
            nodes: vec![DescribedNode {
                depth: 0,
                role: "CheckBox".to_owned(),
                label: "Buy milk".to_owned(),
                value: String::new(),
                toggled: Some(Toggled::On),
                disabled: false,
                focused: true,
                rect: Some(ChildRect {
                    x: 8.0,
                    y: 48.0,
                    width: 200.0,
                    height: 24.0,
                }),
            }],
            test_ids: vec![TestIdRect {
                id: "checklist.add".to_owned(),
                rect: ChildRect {
                    x: 8.0,
                    y: 80.0,
                    width: 60.0,
                    height: 24.0,
                },
            }],
            actions: vec![DescribedAction {
                id: "checklist.clear".to_owned(),
                label: "Clear completed".to_owned(),
                shortcut: Some("Ctrl+Shift+K".to_owned()),
                enabled: false,
                checked: None,
                live: true,
            }],
        }),
    }]);
    assert_eq!(
        decode_frame(&encode_frame(&frames).unwrap()).unwrap(),
        frames
    );
}
