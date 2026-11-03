use super::*;

#[test]
fn a_programs_window_given_a_parent_later_becomes_its_dialog_and_back() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let window =
        |id: u64, title: &str| program_window(id, "gtk3-demo", title, WindowOrigin::default());
    let main = window(1, "GTK+ Demo");
    let dialog = window(2, "Dialogs and Message Boxes");
    let dialog_id = format!("dock.dialog.{}.header", window_tab(2));
    let tab_id = format!("dock.tab.{}", window_tab(2));
    fixture.show_windows(&[main.clone(), dialog.clone()]);
    assert!(
        fixture.test.shown(&tab_id),
        "it starts as a window of its own"
    );

    let child = HostWindow {
        parent: Some(HostWindowId(1)),
        ..dialog.clone()
    };
    fixture.show_windows(&[main.clone(), child.clone()]);
    assert!(
        fixture.test.shown(&dialog_id) && !fixture.test.shown(&tab_id),
        "given a parent, it becomes the parent's dialog"
    );

    fixture.show_windows(&[main.clone(), dialog]);
    assert!(
        fixture.test.shown(&tab_id) && !fixture.test.shown(&dialog_id),
        "its parent cleared, it is a window of its own again"
    );

    fixture.show_windows(&[child.clone()]);
    assert!(
        fixture.test.shown(&tab_id),
        "a window whose parent is not listed is shown on its own rather than lost"
    );

    fixture.show_windows(&[main, child]);
    assert!(
        fixture.test.shown(&dialog_id) && !fixture.test.shown(&tab_id),
        "once its parent is listed, it becomes the parent's dialog"
    );
}
