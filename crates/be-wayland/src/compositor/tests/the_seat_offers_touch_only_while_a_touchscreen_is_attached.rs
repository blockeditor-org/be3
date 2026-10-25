use super::*;
use wayland_client::protocol::wl_seat::Capability;

#[test]
fn the_seat_offers_touch_only_while_a_touchscreen_is_attached() {
    let mut harness = Harness::new();
    harness.settle();
    let offers_touch = |harness: &Harness| {
        harness
            .client
            .received
            .protocols
            .seat_capabilities
            .expect("the seat told its capabilities")
            .contains(Capability::Touch)
    };
    assert!(
        !offers_touch(&harness),
        "with no touchscreen the seat has no touch"
    );

    harness.attach_touchscreen(true);
    assert!(offers_touch(&harness), "a touchscreen gives the seat touch");

    harness.attach_touchscreen(false);
    assert!(
        !offers_touch(&harness),
        "unplugging it takes touch away again"
    );
}
