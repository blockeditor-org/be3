use super::*;

#[test]
fn a_programs_dialog_opens_at_its_listed_size_before_its_status_is_heard() {
    let (mut fixture, _) = editor();
    fixture.show_windows(&[window(1, "Text Editor", None)]);
    let dialog = TestWindow {
        size: be_sdk_beui::beui::Vec2::new(420.0, 160.0),
        modal: true,
        ..window(2, "Unsaved changes", Some(1))
    };
    fixture.windows = vec![window(1, "Text Editor", None), dialog.clone()];
    let listed: Vec<be_sdk_beui::HostWindow> =
        fixture.windows.iter().map(TestWindow::listed).collect();
    fixture.host.set_watched::<WindowsWatch>(&listed);
    fixture.test.run();

    let placed = placed_windows(&fixture)
        .into_iter()
        .find(|(id, _)| *id == 2)
        .map(|(_, rect)| rect)
        .expect("the dialog is placed in the frame it is listed");
    assert_eq!(
        (placed.width(), placed.height()),
        (420.0, 160.0),
        "the dialog's first frame is at the size its program picked, not a placeholder's"
    );
    fixture.settle();
    fixture
        .test
        .snapshot("a_programs_dialog_opens_at_its_listed_size_before_its_status_is_heard");
}
