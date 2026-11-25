use super::*;

#[test]
fn a_hosted_modal_dock_dialog_keeps_the_focus_and_gives_it_back_to_its_tab() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness, open, ..
    } = hosted_dock(WIDE_VIEWPORT);
    open.set(false);
    harness.frame(Vec::new());
    let under = harness.find("page.button");
    harness.click(harness.rect(under).center());
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused, Some(under));

    open.set(true);
    for _ in 0..3 {
        harness.frame(Vec::new());
    }
    let inside = harness.find("dialog.button");
    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
        let focused = harness.document().focused;
        assert_ne!(focused, Some(under), "Tab never reaches the blocked panel");
        assert!(
            focused
                .is_some_and(|node| harness.document().window_of(node)
                    == harness.document().window_of(inside)),
            "it stays in the dialog's window: {focused:?}"
        );
    }

    open.set(false);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused,
        Some(under),
        "closed, it gives the focus back to its tab's control in the main window"
    );
}
