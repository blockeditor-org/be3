use super::*;

#[test]
fn a_childs_dock_views_keep_their_screens_while_placed() {
    let child = up(1, ChildMode::Live);
    let editor = EditorInstanceId(1);
    let tab = |tab| child.shared.tab_screen(editor, tab);
    let first = tab(4);
    let second = tab(6);
    assert_eq!(
        (first, second),
        (screen_of(editor, 3), screen_of(editor, 4))
    );
    // Asked again, each keeps its screen; one no longer asked lets its screen go, and a new
    // one never takes a screen still held.
    assert_eq!(tab(4), first);
    child.shared.tabs.borrow_mut().retain(|tab| tab.tab != 4);
    assert_eq!(tab(8), screen_of(editor, 5));
    assert_eq!(tab(6), second);
    // Each editor's screens are its own, numbered from its instance.
    assert_ne!(child.shared.tab_screen(EditorInstanceId(2), 4), first);
}
