use super::*;

#[test]
fn a_hosted_dock_dialog_is_a_window_of_its_own_beside_its_tab_that_its_host_moves() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness,
        window,
        at,
        pressed,
        ..
    } = hosted_dock(WIDE_VIEWPORT);
    let root = harness.document().root().expect("the dock built a root");
    let tab = harness.rect(dock_tab(harness.document(), root, "Page"));
    assert!(
        at.y >= tab.bottom(),
        "below the tab it hangs from: {at:?} under {tab:?}"
    );

    let page = harness.rect(harness.find("page")).center();
    harness.click(page);
    harness.frame(Vec::new());
    assert_eq!(
        pressed.get_untracked(),
        0,
        "a modal dialog in a window of its own blocks its tab's panel in the main window"
    );

    let header = harness.rect(harness.find("dock.dialog.2.header")).center();
    press_in(&mut harness, window, header, true);
    harness.frame(Vec::new());
    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::BeginMove(window)),
        "its bar asks its host to move it"
    );
    press_in(&mut harness, window, header, false);
}
