use super::*;
use crate::reactive::*;

#[test]
fn a_narrow_tab_trades_places_with_a_wide_one_once_it_covers_half_of_it() {
    let _root = RootScope::new();
    let keys = create_signal(vec![0_usize, 1, 2]);
    let mut harness = Harness::new(tab_strip(keys));
    harness.settle();
    let grab = harness.center(harness.find("tab.1"));
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab - vec2(6.0, 0.0))]);
    let wide = |harness: &mut Harness, by: f32| {
        drag_tab_to(harness, grab, by);
        tab_left(harness, 0)
    };
    assert_eq!(
        wide(&mut harness, -43.0),
        0.0,
        "the narrow tab's left edge (41) is short of the wide tab's middle (40)"
    );
    assert_eq!(
        wide(&mut harness, -45.0),
        24.0,
        "once it passes that middle, having covered half the wide tab, the wide tab moves after it"
    );
    assert_eq!(
        wide(&mut harness, -41.0),
        24.0,
        "coming back, they stay while its right edge (63) is short of the wide tab's new middle (64)"
    );
    assert_eq!(
        wide(&mut harness, -39.0),
        0.0,
        "and trade back once it passes it"
    );
    wide(&mut harness, -45.0);
    harness.release_at(grab - vec2(45.0, 0.0));
    assert_eq!(keys.get_untracked(), vec![1, 0, 2]);
    harness.settle();
    assert_eq!(tab_left(&harness, 1), 0.0);
}
