use super::*;

#[test]
fn a_programs_modal_dialog_points_at_its_tab_and_dims_its_window() {
    let (mut fixture, _) = editor();
    let drawing = |id: u64, title: &str, parent: Option<u64>| block_editor_beui::HostWindow {
        app_id: "com.github.maoschanz.drawing".to_owned(),
        name: "Drawing".to_owned(),
        ..window(id, title, parent)
    };
    let dialog = block_editor_beui::HostWindow {
        modal: true,
        ..drawing(2, "com.github.maoschanz.drawing", Some(1))
    };
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![drawing(1, "Untitled image", None), dialog]);
    fixture.settle();

    let tab = (1u64 << 41) + 2;
    let header = fixture.test.rect_of(&format!("dock.dialog.{tab}.header"));
    let parent_tab = fixture
        .test
        .rect_of(&format!("dock.tab.{}", (1u64 << 41) + 1));
    assert!(
        header.top() > parent_tab.bottom()
            && header.left() <= parent_tab.center().x
            && parent_tab.center().x <= header.right(),
        "the dialog hangs from the tab of the window it came from: {header:?} below {parent_tab:?}"
    );
    assert!(
        fixture.says("Drawing") && !fixture.says("com.github.maoschanz.drawing"),
        "its header names the program rather than its id"
    );
    fixture
        .test
        .snapshot("a_programs_modal_dialog_points_at_its_tab_and_dims_its_window");
}
