use super::*;

#[test]
fn a_press_in_a_menu_closes_its_submenu_and_one_outside_closes_both() {
    // A menu opened from a button, and a submenu hanging from it beside it.
    let mut menu = hung(
        1,
        None,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 200.0)),
    );
    menu.trigger = Some(Rect::from_min_max(pos2(0.0, 210.0), pos2(40.0, 240.0)));
    let submenu = hung(
        2,
        Some(1),
        Rect::from_min_max(pos2(100.0, 40.0), pos2(200.0, 140.0)),
    );
    let shown = [menu, submenu];
    // In the submenu: nothing closes.
    assert!(dismissed_by_press(&shown, pos2(150.0, 60.0)).is_empty());
    // In the menu, outside the submenu: the submenu closes, the menu stays.
    assert_eq!(
        dismissed_by_press(&shown, pos2(50.0, 150.0)),
        vec![ViewId(2)]
    );
    // On the menu's trigger: the trigger's own click closes it, so the press closes only the
    // submenu, which the trigger is outside of.
    assert_eq!(
        dismissed_by_press(&shown, pos2(20.0, 220.0)),
        vec![ViewId(2)]
    );
    // Outside both: both close.
    assert_eq!(
        dismissed_by_press(&shown, pos2(300.0, 300.0)),
        vec![ViewId(1), ViewId(2)]
    );
}
