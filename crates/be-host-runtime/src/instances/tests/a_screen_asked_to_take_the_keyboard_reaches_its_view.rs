use super::*;

#[test]
fn a_screen_asked_to_take_the_keyboard_reaches_its_view() {
    let (_mailbox, heard) = hearing();
    let mut instances = placed();
    instances.next_screens(PASS);
    let screen = instances.entries[&INSTANCE].screens[&REGION].request.screen;

    instances.request_focus(screen);
    host::deliver();
    let asked: Vec<ViewId> = heard
        .borrow()
        .iter()
        .filter_map(|event| match event {
            InstanceEvent::FocusRequested(view) => Some(*view),
            _ => None,
        })
        .collect();
    assert_eq!(
        asked,
        [REGION],
        "the view that shows the screen hears it, once"
    );
}
