use be_sdk_beui::HostNotificationAction;
use be_sdk_beui::beui::accesskit::{Node, Toggled};

use super::*;

fn described(fixture: &Fixture, test_id: &str) -> Node {
    let document = fixture.test.document();
    let node = document
        .find_test_id(test_id)
        .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
    document
        .accessibility
        .get(&node)
        .cloned()
        .unwrap_or_else(|| panic!("{test_id:?} has no accessibility node"))
}

fn state(fixture: &Fixture, test_id: &str) -> (Option<Toggled>, Option<String>) {
    let node = described(fixture, test_id);
    (node.toggled(), node.description().map(str::to_owned))
}

#[test]
fn window_chips_and_toasts_tell_a_screen_reader_their_state() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1)]);
    let mut fixture = Fixture::resumed(before.saved());
    fixture.show_windows(&[
        HostWindow {
            focused: true,
            ..notes(2)
        },
        HostWindow {
            shown: false,
            ..host_window(3, false)
        },
    ]);
    assert_eq!(
        state(&fixture, "desktop.windows.2"),
        (Some(Toggled::True), None),
        "the focused window's chip reads as pressed"
    );
    assert_eq!(
        state(&fixture, "desktop.windows.3"),
        (Some(Toggled::False), Some("Hidden".to_owned())),
        "a hidden window's chip says it is hidden"
    );
    assert_eq!(
        state(&fixture, "desktop.windows.placeholder.1"),
        (Some(Toggled::False), Some("Not running".to_owned())),
        "a chip kept from last time says its program is not running"
    );

    fixture.notify(&[IncomingNotification {
        actions: vec![HostNotificationAction {
            key: DEFAULT_ACTION.to_owned(),
            label: "Open".to_owned(),
        }],
        ..notification(1, "Mail", "Mia Chen", "Lunch at noon?")
    }]);
    let activate = described(&fixture, &format!("{}.activate", toast(1)));
    assert_eq!(
        (activate.label(), activate.description()),
        (Some("Mia Chen"), Some("Lunch at noon?")),
        "a toast's button is named by its title and described by its message"
    );
}
