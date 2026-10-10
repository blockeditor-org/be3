use super::*;

#[test]
fn a_window_with_a_parent_floats_at_the_size_it_drew() {
    let (mut fixture, _) = editor();

    fixture.host.set_host_value::<HostWindows>(&vec![
        window(1, "Editor", None),
        window(2, "Save changes?", Some(1)),
    ]);
    fixture.settle();

    let placed = placed_windows(&fixture);
    assert_eq!(placed.len(), 2, "both windows are placed");
    let dialog = placed
        .iter()
        .find(|(id, _)| *id == 2)
        .map(|(_, rect)| *rect)
        .expect("the dialog is placed");
    assert_eq!(
        (dialog.width(), dialog.height()),
        (320.0, 200.0),
        "the dialog floats at the size it drew rather than filling a pane"
    );
}
