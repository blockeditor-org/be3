use be_sdk_beui::beui::PointerButton;

use super::*;

fn monitor_entry(id: u64, name: &str) -> TestWindow {
    TestWindow {
        kind: WindowKind::Display,
        title: name.to_owned(),
        program_id: String::new(),
        name: String::new(),
        size: Vec2::new(2560.0, 1440.0),
        ..host_window(id, false)
    }
}

#[test]
fn the_windows_bar_shows_a_chip_per_window_and_shows_hides_or_closes_it() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let dialog = TestWindow {
        parent: Some(HostWindowId(1)),
        title: "Preferences".to_owned(),
        ..foot(4)
    };
    fixture.show_windows(&[
        TestWindow {
            focused: true,
            ..foot(1)
        },
        notes(2),
        monitor_entry(3, "DELL U2720Q"),
        dialog,
    ]);
    fixture.host.show_block(be_sdk_beui::ShowRequest {
        block_id: Uuid::new_v4(),
        block_type: SHOWN_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();
    assert!(fixture.window_rect(1).is_some(), "a window is drawn");
    assert!(fixture.window_rect(3).is_none(), "a display opens hidden");
    let block = fixture.test.test_ids();
    let block = block
        .iter()
        .map(|(id, _)| id)
        .find_map(|id| {
            id.strip_prefix("desktop.windows.block.")
                .filter(|rest| !rest.contains('.'))
        })
        .expect("the block has a chip")
        .to_owned();

    let chip = |fixture: &Fixture, id: &str| {
        fixture
            .test
            .rect_of(&format!("desktop.windows.{id}"))
            .left()
    };
    assert!(
        chip(&fixture, "3") < chip(&fixture, "1")
            && chip(&fixture, "1") < chip(&fixture, "2")
            && chip(&fixture, "2") < chip(&fixture, &format!("block.{block}")),
        "displays come first, then program windows in the order they came, then the shell's own"
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

    fixture
        .test
        .click_with("desktop.windows.1", PointerButton::Secondary);
    fixture.settle();
    assert!(fixture.test.shown("desktop.windows.1.visibility"));
    assert!(fixture.test.shown("desktop.windows.1.close"));
    fixture.test.snapshot("a_window_chips_menu");
    fixture.test.click("desktop.windows.1.visibility");
    fixture.settle();
    assert!(
        fixture.window_rect(1).is_none(),
        "the menu hides the window, which is then placed nowhere"
    );
    assert!(
        fixture.test.shown("desktop.windows.1"),
        "a hidden window keeps its chip"
    );
    assert!(fixture.says("Terminal 1"), "and its name");

    fixture.focused_windows();
    fixture.test.click("desktop.windows.1");
    fixture.settle();
    assert!(fixture.window_rect(1).is_some(), "its chip shows it again");
    assert!(
        fixture.focused_windows().contains(&HostWindowId(1)),
        "and gives it the keyboard"
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

    fixture.test.click_with(
        &format!("desktop.windows.block.{block}"),
        PointerButton::Secondary,
    );
    fixture.settle();
    fixture
        .test
        .click(&format!("desktop.windows.block.{block}.close"));
    fixture.settle();
    assert!(
        !fixture
            .test
            .shown(&format!("desktop.windows.block.{block}")),
        "closing a block's window from its chip closes it"
    );
    assert!(fixture.placed_blocks().is_empty());

    fixture
        .test
        .click_with("desktop.windows.2", PointerButton::Secondary);
    fixture.settle();
    fixture.test.click("desktop.windows.2.close");
    fixture.settle();
    assert_eq!(
        fixture.closed_windows(),
        vec![HostWindowId(2)],
        "a chip's Close asks the window's program to close it"
    );
    assert!(
        fixture.test.shown("desktop.windows.2"),
        "and the chip stays until the window closes"
    );
    fixture.show_windows(&[foot(1), monitor_entry(3, "DELL U2720Q")]);
    assert!(!fixture.test.shown("desktop.windows.2"));
}
