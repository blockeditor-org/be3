use super::*;

/// An ask for a host panel its placer withdraws before the root drew it ends: the panel placed
/// and drawn again later does not take the keyboard from where it went meanwhile.
#[test]
fn a_keyboard_ask_for_a_host_panel_withdrawn_before_it_is_drawn_ends() {
    let mut children = placing(Vec::new());
    children.panels.push(panel(false));
    ask(&mut children, 9, 1);
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);

    children.panels.clear();
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);

    children.panels.push(panel(true));
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);
}
