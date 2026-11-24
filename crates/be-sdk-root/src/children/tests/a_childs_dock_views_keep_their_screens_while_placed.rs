use super::*;

#[test]
fn a_childs_dock_views_keep_their_screens_while_placed() {
    let mut child = up(1, ChildMode::Live);
    let first = child.tab_screen(4);
    let second = child.tab_screen(6);
    assert_eq!((first, second), (ScreenId(3), ScreenId(4)));
    // Asked again, each keeps its screen; one no longer asked lets its screen go, and a new
    // one never takes a screen still held.
    assert_eq!(child.tab_screen(4), first);
    child.tabs.retain(|tab| tab.tab != 4);
    assert_eq!(child.tab_screen(8), ScreenId(5));
    assert_eq!(child.tab_screen(6), second);
}
