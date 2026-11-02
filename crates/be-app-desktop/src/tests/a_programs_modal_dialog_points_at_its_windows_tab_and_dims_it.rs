use super::*;

#[test]
fn a_programs_modal_dialog_points_at_its_windows_tab_and_dims_it() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let drawing = |id: u64, title: &str| HostWindow {
        name: "Drawing".to_owned(),
        ..program_window(
            id,
            "com.github.maoschanz.drawing",
            title,
            WindowOrigin::default(),
        )
    };
    let dialog = HostWindow {
        parent: Some(HostWindowId(1)),
        modal: true,
        ..drawing(2, "com.github.maoschanz.drawing")
    };
    fixture.show_windows(&[drawing(1, "Untitled image"), dialog]);

    let header = fixture
        .test
        .rect_of(&format!("dock.dialog.{}.header", window_tab(2)));
    let tab = fixture.test.rect_of(&format!("dock.tab.{}", window_tab(1)));
    assert!(
        header.top() > tab.bottom()
            && header.left() <= tab.center().x
            && tab.center().x <= header.right(),
        "the dialog hangs from the tab of the window it came from: {header:?} below {tab:?}"
    );
    assert!(
        fixture.says("Drawing") && !fixture.says("com.github.maoschanz.drawing"),
        "its header names the program rather than its id"
    );
    fixture
        .test
        .snapshot("a_programs_modal_dialog_points_at_its_windows_tab_and_dims_it");
}
