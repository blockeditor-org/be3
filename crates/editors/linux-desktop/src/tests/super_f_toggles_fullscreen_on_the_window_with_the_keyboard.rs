use block_plugin_api::ChildRect;

use super::*;

fn fullscreen_asked(fixture: &Fixture) -> Vec<WindowAction> {
    fixture
        .test
        .actions::<WindowAction>()
        .into_iter()
        .filter(|action| matches!(action, WindowAction::Fullscreen { .. }))
        .collect()
}

#[test]
fn super_f_toggles_fullscreen_on_the_window_with_the_keyboard() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    assert!(
        fixture.test.app_key(Modifiers::LOGO, Key::F),
        "the desktop takes Super+F before the program that has the keyboard"
    );
    fixture.settle();
    assert_eq!(
        fullscreen_asked(&fixture),
        vec![WindowAction::Fullscreen {
            window: HostWindowId(2),
            fullscreen: true
        }]
    );

    fixture.test.take_sent();
    let fullscreen = HostWindow {
        fullscreen: Some(ChildRect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        }),
        ..host_window(2, true)
    };
    fixture
        .test
        .set_host_value::<HostWindows>(&vec![host_window(1, false), fullscreen]);
    fixture.settle();
    fixture.test.take_sent();
    assert!(fixture.test.app_key(Modifiers::LOGO, Key::F));
    fixture.settle();
    assert_eq!(
        fullscreen_asked(&fixture),
        vec![WindowAction::Fullscreen {
            window: HostWindowId(2),
            fullscreen: false
        }],
        "a second Super+F takes it out again"
    );
}
