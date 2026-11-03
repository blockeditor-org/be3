use beui::{TouchPhase, WindowEdge};
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;

use super::*;
use crate::state::Grab;

#[test]
fn a_move_asked_from_a_finger_held_on_the_window_begins_one() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Carried windows />
        }
    });
    harness.attach_touchscreen(true);
    let _touch = harness.client.touch();
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    let toplevel = window.toplevel.clone().expect("the window is a toplevel");
    let seat = harness.client.seat.clone().expect("the client has a seat");
    let at = harness.window(id).min + Vec2::new(40.0, 10.0);

    harness.frame(vec![finger(0, TouchPhase::Start, at)]);
    harness.settle();
    let serial = harness
        .client
        .received
        .serial
        .expect("the finger reached the program");
    toplevel._move(&seat, serial);
    harness.settle();
    assert_eq!(
        begun(),
        [Grab::Move],
        "a move asked with the serial of a finger still held begins one"
    );
    toplevel.resize(&seat, serial, ResizeEdge::BottomLeft);
    harness.settle();
    assert_eq!(
        begun(),
        [Grab::Resize(WindowEdge::BottomLeft)],
        "so does a resize"
    );
    toplevel._move(&seat, serial.wrapping_add(1000));
    harness.settle();
    assert!(
        begun().is_empty(),
        "a move asked with a serial no held finger or button has carries nothing"
    );

    harness.frame(vec![finger(0, TouchPhase::End, at)]);
    harness.settle();
    toplevel._move(&seat, serial);
    harness.settle();
    assert!(
        begun().is_empty(),
        "a move asked after the finger was lifted carries nothing"
    );
}
