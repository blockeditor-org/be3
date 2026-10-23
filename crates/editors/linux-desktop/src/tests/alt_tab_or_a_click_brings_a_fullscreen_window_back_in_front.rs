use block_editor_beui::beui::{Pos2, pos2};
use block_plugin_api::ChildRect;

use super::*;

const SCREEN: ChildRect = ChildRect {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn windows(fixture: &mut Fixture, focused: u64) {
    let calculator = HostWindow {
        fullscreen: Some(SCREEN),
        ..host_window(1, focused == 1)
    };
    fixture
        .test
        .set_host_value::<HostWindows>(&vec![calculator, host_window(2, focused == 2)]);
    fixture.settle();
}

fn front_at(fixture: &Fixture, at: Pos2) -> Option<HostWindowId> {
    let occluders = fixture.test.occluders();
    fixture
        .test
        .children()
        .iter()
        .enumerate()
        .rev()
        .filter(|(index, placement)| {
            placement.clip.contains(at.x, at.y)
                && !occluders.iter().any(|occluder| {
                    occluder.after as usize > *index && occluder.rect.contains(at.x, at.y)
                })
        })
        .find_map(|(_, placement)| match placement.content {
            ChildContent::Window(window) => Some(window),
            _ => None,
        })
}

#[test]
fn alt_tab_or_a_click_brings_a_fullscreen_window_back_in_front() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    windows(&mut fixture, 1);
    let terminal = fixture
        .test
        .children()
        .iter()
        .find(|placement| placement.content == ChildContent::Window(HostWindowId(2)))
        .map(|placement| {
            pos2(
                placement.rect.x + placement.rect.width / 2.0,
                placement.rect.y + placement.rect.height / 2.0,
            )
        })
        .expect("the terminal is placed");
    assert_eq!(
        front_at(&fixture, terminal),
        Some(HostWindowId(1)),
        "the fullscreen calculator covers the terminal"
    );

    windows(&mut fixture, 2);
    assert_eq!(
        front_at(&fixture, terminal),
        Some(HostWindowId(2)),
        "the focused terminal comes in front of it"
    );
    fixture.test.take_sent();

    assert!(fixture.test.app_key(Modifiers::ALT, Key::Tab));
    fixture.settle();
    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
    assert_eq!(fixture.focused_windows(), [HostWindowId(1)]);
    windows(&mut fixture, 1);
    assert_eq!(
        front_at(&fixture, terminal),
        Some(HostWindowId(1)),
        "Alt+Tab back brings the fullscreen calculator in front, not its empty tab"
    );

    windows(&mut fixture, 2);
    fixture.test.take_sent();
    fixture
        .test
        .click(pos2(SCREEN.width - 10.0, SCREEN.height - 10.0));
    fixture.settle();
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(1)],
        "a click on the fullscreen calculator focuses it"
    );
    assert_eq!(
        front_at(&fixture, terminal),
        Some(HostWindowId(1)),
        "and brings it in front of the terminal"
    );
}
