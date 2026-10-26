use super::*;

fn monitor_entry(id: u64, name: &str) -> HostWindow {
    HostWindow {
        kind: WindowKind::Display,
        shown: false,
        title: name.to_owned(),
        app_id: String::new(),
        size: Vec2::new(2560.0, 1440.0),
        ..host_window(id, false)
    }
}

#[test]
fn the_window_picker_lists_displays_then_windows_and_shows_or_hides_them() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let hidden_notes = HostWindow {
        shown: false,
        ..notes(2)
    };
    let dialog = HostWindow {
        parent: Some(HostWindowId(1)),
        title: "Preferences".to_owned(),
        ..host_window(4, false)
    };
    fixture.show_windows(&[
        HostWindow {
            focused: true,
            ..foot(1)
        },
        hidden_notes.clone(),
        monitor_entry(3, "DELL U2720Q"),
        dialog,
    ]);
    assert!(fixture.window_rect(1).is_some(), "a shown window is drawn");
    assert!(fixture.window_rect(2).is_none(), "a hidden window is not");
    assert!(fixture.window_rect(3).is_none(), "nor a hidden display");

    fixture.test.click("desktop.windows");
    fixture.settle();
    let row = |fixture: &Fixture, id: u64| {
        fixture
            .test
            .rect_of(&format!("desktop.windows.entry.{id}"))
    };
    assert!(
        row(&fixture, 3).top() < row(&fixture, 1).top()
            && row(&fixture, 1).top() < row(&fixture, 2).top(),
        "displays come first, then windows in the host's order"
    );
    assert!(
        !fixture.test.shown("desktop.windows.entry.4"),
        "a dialog goes with its window instead of being listed"
    );
    assert!(fixture.test.shown("desktop.windows.entry.1.hide"));
    assert!(
        !fixture.test.shown("desktop.windows.entry.2.hide"),
        "only a shown entry can be hidden"
    );
    fixture.test.snapshot("the_window_picker");

    fixture.test.clear_calls();
    fixture.test.click("desktop.windows.entry.2");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::window),
        [
            WindowAction::Show(HostWindowId(2)),
            WindowAction::Focus(HostWindowId(2))
        ],
        "a hidden window is shown and given the keyboard"
    );
    assert!(
        !fixture.test.shown("desktop.windows.panel"),
        "choosing an entry closes the picker"
    );

    fixture.test.click("desktop.windows");
    fixture.settle();
    fixture.test.click("desktop.windows.entry.1.hide");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::window),
        [WindowAction::Hide(HostWindowId(1))]
    );

    fixture.show_windows(&[
        HostWindow {
            shown: false,
            ..foot(1)
        },
        HostWindow {
            shown: true,
            ..hidden_notes
        },
        monitor_entry(3, "DELL U2720Q"),
    ]);
    assert!(fixture.window_rect(1).is_none(), "the host hid the window");
    assert!(fixture.window_rect(2).is_some(), "and showed the other");
}
