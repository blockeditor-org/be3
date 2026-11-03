use super::*;

#[test]
fn a_dialog_follows_its_parent_into_the_placeholder_it_claims() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), foot(2)]);
    let mut resumed = Fixture::resumed(before.saved());
    let settings = HostWindow {
        parent: Some(HostWindowId(40)),
        ..foot(41)
    };
    let dialog = HostWindow {
        parent: Some(HostWindowId(41)),
        modal: true,
        ..foot(42)
    };
    resumed.show_windows(&[foot(40), settings, dialog.clone()]);
    assert!(
        resumed.placeholder_rect(2).is_some(),
        "the second placeholder waits while its terminal is a dialog"
    );
    resumed.show_windows(&[foot(40), foot(41), dialog]);

    assert!(
        resumed.placeholder_rect(2).is_none(),
        "the window that stopped being a dialog claims the other placeholder"
    );
    let header = resumed
        .test
        .rect_of(&format!("dock.dialog.{}.header", window_tab(42)));
    let tab = resumed.test.rect_of(&format!("dock.tab.{}", window_tab(2)));
    assert!(
        header.top() > tab.bottom()
            && header.left() <= tab.center().x
            && tab.center().x <= header.right(),
        "the dialog hangs from the tab its parent took over: {header:?} below {tab:?}"
    );
}
