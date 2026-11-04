use super::*;
use crate::reactive::*;

#[test]
fn a_wide_tab_trades_places_with_a_narrow_one_once_it_covers_half_of_it() {
    let _root = RootScope::new();
    let keys = create_signal(vec![0_usize, 1, 2]);
    let mut harness = Harness::new(tab_strip(keys));
    harness.settle();
    let grab = harness.center(harness.find("tab.0"));
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(6.0, 0.0))]);
    assert_eq!(
        drag_tab_to(&mut harness, grab, 13.0),
        84.0,
        "the wide tab's right edge (93) is short of the narrow tab's middle (94)"
    );
    assert_eq!(
        drag_tab_to(&mut harness, grab, 15.0),
        0.0,
        "once its right edge passes that middle, the narrow tab moves to the front"
    );
    assert_eq!(
        drag_tab_to(&mut harness, grab, 11.0),
        0.0,
        "coming back, they stay while the wide tab's left edge (11) is past the narrow tab's middle (10)"
    );
    assert_eq!(
        drag_tab_to(&mut harness, grab, 9.0),
        84.0,
        "and trade back once it is not"
    );
    drag_tab_to(&mut harness, grab, 15.0);
    harness.release_at(grab + vec2(15.0, 0.0));
    assert_eq!(keys.get_untracked(), vec![1, 0, 2]);
}
