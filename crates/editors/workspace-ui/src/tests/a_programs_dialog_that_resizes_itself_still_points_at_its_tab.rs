use super::*;

#[test]
fn a_programs_dialog_that_resizes_itself_still_points_at_its_tab() {
    let (mut fixture, _) = editor();
    fixture.host.set_watched::<WindowsWatch>(&vec![
        window(1, "Text Editor", None),
        window(2, "Preferences", Some(1)),
    ]);
    fixture.settle();
    let grown = block_editor_beui::HostWindow {
        size: block_editor_beui::beui::Vec2::new(420.0, 260.0),
        ..window(2, "Preferences", Some(1))
    };
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![window(1, "Text Editor", None), grown]);
    fixture.settle();

    let dialog = placed_windows(&fixture)
        .into_iter()
        .find(|(id, _)| *id == 2)
        .map(|(_, rect)| rect)
        .expect("the dialog is placed");
    assert_eq!(
        (dialog.width(), dialog.height()),
        (420.0, 260.0),
        "the dialog follows the size its program chose"
    );
    let parent_tab = fixture
        .test
        .rect_of(&format!("dock.tab.{}", (1u64 << 41) + 1));
    assert!(
        dialog.top() > parent_tab.bottom()
            && dialog.left() <= parent_tab.center().x
            && parent_tab.center().x <= dialog.right(),
        "and still hangs from its parent's tab: {dialog:?} below {parent_tab:?}"
    );
    fixture
        .test
        .snapshot("a_programs_dialog_that_resizes_itself_still_points_at_its_tab");
}
