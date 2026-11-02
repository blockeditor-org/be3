use crate::test_client::Seen;

use super::*;

const KEY_ESC: u32 = 1;

#[test]
fn a_menubar_moving_to_its_next_menu_keeps_the_keyboard_on_it() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    let serial = harness.client.received.serial;

    let file = harness.client.grabbing_popup(
        harness.stepped.program_mut().compositor.server(),
        &window,
        (0, 10),
        (40, 30),
    );
    harness.settle();
    assert_eq!(
        harness.client.received.keyboard_surface.as_ref(),
        Some(&file.surface)
    );
    harness.client.request_token(serial, Some(&file.surface));
    harness.settle();
    let token = harness
        .client
        .received
        .token
        .take()
        .expect("a client with a grabbing menu is given a token");

    file.destroy();
    harness.client.sync();
    let edit = harness.client.grabbing_popup(
        harness.stepped.program_mut().compositor.server(),
        &window,
        (40, 10),
        (40, 30),
    );
    harness.settle();
    assert_eq!(
        harness.client.received.keyboard_surface.as_ref(),
        Some(&edit.surface),
        "the next menu, grabbing with the serial of the first click, takes the keyboard"
    );

    assert!(
        harness
            .client
            .received
            .seen
            .contains(&Seen::Synced(Some(window.surface.clone()))),
        "the keyboard is back on the window as soon as its menu is destroyed, before the next \
         request is answered; a client that finds no surface of its own focused then thinks it \
         lost the focus and closes every menu"
    );

    harness.client.activate(&token, &window.surface);
    harness.settle();
    assert_eq!(
        harness.client.received.keyboard_surface.as_ref(),
        Some(&edit.surface),
        "activating the window that owns the grab leaves the keyboard on its menu"
    );
    assert!(
        !harness
            .client
            .received
            .seen
            .iter()
            .any(|seen| matches!(seen, Seen::PopupDone(_))),
        "no menu is dismissed"
    );

    harness.compositor().server().state.key(KEY_ESC, true);
    harness.settle();
    assert_eq!(
        harness.client.received.seen.last(),
        Some(&Seen::Key(edit.surface.clone(), KEY_ESC)),
        "keys reach the open menu"
    );
}
