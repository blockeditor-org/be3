use block_editor_beui::beui::PointerButton;

use super::*;

fn monitor_entry(id: u64, name: &str) -> HostWindow {
    HostWindow {
        kind: WindowKind::Display,
        shown: false,
        title: name.to_owned(),
        app_id: String::new(),
        name: String::new(),
        size: Vec2::new(2560.0, 1440.0),
        ..host_window(id, false)
    }
}

#[test]
fn the_windows_bar_shows_a_chip_per_window_and_shows_hides_or_closes_it() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let hidden_notes = HostWindow {
        shown: false,
        ..notes(2)
    };
    let dialog = HostWindow {
        parent: Some(HostWindowId(1)),
        title: "Preferences".to_owned(),
        ..foot(4)
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
    fixture.host.show_block(block_editor_beui::ShowRequest {
        block_id: Uuid::new_v4(),
        block_type: SHOWN_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();
    assert!(fixture.window_rect(1).is_some(), "a shown window is drawn");
    assert!(fixture.window_rect(2).is_none(), "a hidden window is not");
    assert!(fixture.window_rect(3).is_none(), "nor a hidden display");

    let chip = |fixture: &Fixture, id: &str| {
        fixture
            .test
            .rect_of(&format!("desktop.windows.{id}"))
            .left()
    };
    assert!(
        chip(&fixture, "3") < chip(&fixture, "1")
            && chip(&fixture, "1") < chip(&fixture, "2")
            && chip(&fixture, "2") < chip(&fixture, "block.2"),
        "displays come first, then program windows in the host's order, then the shell's own"
    );
    assert!(
        !fixture.test.shown("desktop.windows.4"),
        "a dialog goes with its window instead of having a chip"
    );
    assert!(
        fixture.says("Terminal 1"),
        "a chip on a horizontal bar names its window"
    );
    fixture.test.snapshot("the_windows_bar");

    fixture.test.clear_calls();
    fixture.test.click("desktop.windows.2");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::window),
        [
            WindowAction::Show(HostWindowId(2)),
            WindowAction::Focus(HostWindowId(2))
        ],
        "a hidden window's chip shows it and gives it the keyboard"
    );

    fixture
        .test
        .click_with("desktop.windows.1", PointerButton::Secondary);
    fixture.settle();
    assert!(fixture.test.shown("desktop.windows.1.visibility"));
    assert!(fixture.test.shown("desktop.windows.1.close"));
    fixture.test.snapshot("a_window_chips_menu");
    fixture.test.click("desktop.windows.1.visibility");
    fixture.settle();
    let calls = fixture
        .test
        .take_host_calls(block_ui_test::requested::window);
    assert_eq!(
        calls.first(),
        Some(&WindowAction::Hide(HostWindowId(1))),
        "the menu hides the window: {calls:?}"
    );

    fixture
        .test
        .click_with("desktop.windows.3", PointerButton::Secondary);
    fixture.settle();
    assert!(fixture.test.shown("desktop.windows.3.visibility"));
    assert!(
        !fixture.test.shown("desktop.windows.3.close"),
        "a display is shown or hidden, never closed"
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();

    fixture
        .test
        .click_with("desktop.windows.block.2", PointerButton::Secondary);
    fixture.settle();
    fixture.test.click("desktop.windows.block.2.close");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.windows.block.2"),
        "closing a block's window from its chip closes it"
    );
    assert!(fixture.placed_blocks().is_empty());

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
    assert!(
        fixture.test.shown("desktop.windows.1"),
        "a hidden window keeps its chip"
    );
}
