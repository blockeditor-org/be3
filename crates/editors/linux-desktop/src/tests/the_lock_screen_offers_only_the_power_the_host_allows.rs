use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn the_lock_screen_offers_only_the_power_the_host_allows() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 600.0));
    let right = Rect::from_min_size(pos2(500.0, 0.0), vec2(300.0, 400.0));
    let mut test = lock_screen(locked());
    test.set_monitors(vec![("left", left), ("right", right)]);
    test.run();
    assert!(left.contains_rect(test.rect_of("desktop.lock.0")));
    assert!(right.contains_rect(test.rect_of("desktop.lock.1")));
    for index in 0..LOCK_POWER.len() {
        assert!(test.shown(&format!("desktop.lock.0.action.{index}")));
    }
    assert!(
        !test.shown(&format!("desktop.lock.0.action.{}", LOCK_POWER.len())),
        "neither locking again nor logging out is offered"
    );
    test.snapshot("the_desktop_lock_screen_on_two_monitors");

    test.click("desktop.lock.1.action.0");
    test.run();
    assert_eq!(
        test.take_actions::<PowerAction>(),
        vec![PowerAction::Suspend]
    );

    test.set_host_value::<Power>(&PowerAvailability {
        suspend: false,
        ..PowerAvailability::default()
    });
    test.run();
    assert!(
        !test.shown("desktop.lock.0.action.0"),
        "a host that cannot suspend, restart or power off offers no buttons"
    );
}
