use super::*;
use crate::test_client::TouchSeen;
use beui::TouchPhase;

#[test]
fn a_finger_held_on_one_window_stays_on_it_while_it_moves_over_another() {
    let mut harness = Harness::new();
    harness.attach_touchscreen(true);
    let _touch = harness.client.touch();
    let (first, first_id) = harness.open();
    let (second, second_id) = harness.open();
    let held = harness.window(first_id);
    let start = held.center();
    let over = harness.window(second_id).center();
    assert!(!held.contains(over));

    harness.frame(vec![finger(0, TouchPhase::Start, start)]);
    harness.frame(vec![finger(0, TouchPhase::Move, over)]);
    harness.frame(vec![finger(1, TouchPhase::Start, over)]);
    harness.frame(vec![
        finger(0, TouchPhase::End, over),
        finger(1, TouchPhase::End, over),
    ]);
    harness.settle();

    let seen = &harness.client.received.protocols.touch;
    let downs: Vec<(&wayland_client::protocol::wl_surface::WlSurface, i32)> = seen
        .iter()
        .filter_map(|seen| match seen {
            TouchSeen::Down(surface, id, ..) => Some((surface, *id)),
            _ => None,
        })
        .collect();
    assert_eq!(
        downs,
        [(&first.surface, 0), (&second.surface, 1)],
        "each finger goes down on the window under it"
    );
    let moved = (
        f64::from(over.x - held.min.x),
        f64::from(over.y - held.min.y),
    );
    assert!(
        seen.contains(&TouchSeen::Motion(0, moved.0, moved.1)),
        "the first finger keeps moving in its own window's coordinates, past its edge: {seen:?}"
    );
    assert!(
        seen.contains(&TouchSeen::Up(0)) && seen.contains(&TouchSeen::Up(1)),
        "both fingers lift: {seen:?}"
    );
}
