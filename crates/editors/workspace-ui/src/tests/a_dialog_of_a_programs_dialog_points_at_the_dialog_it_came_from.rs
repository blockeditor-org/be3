use super::*;

#[test]
fn a_dialog_of_a_programs_dialog_points_at_the_dialog_it_came_from() {
    let (mut fixture, _) = editor();
    let confirm = block_editor_beui::HostWindow {
        modal: true,
        size: block_editor_beui::beui::Vec2::new(240.0, 120.0),
        ..window(3, "Replace the file?", Some(2))
    };
    fixture.host.set_watched::<WindowsWatch>(&vec![
        window(1, "Editor", None),
        window(2, "Save as", Some(1)),
        confirm,
    ]);
    fixture.settle();

    let header = fixture
        .test
        .rect_of(&format!("dock.dialog.{}.header", (1u64 << 41) + 2));
    let inner = fixture
        .test
        .rect_of(&format!("dock.dialog.{}.header", (1u64 << 41) + 3));
    assert!(
        inner.top() > header.bottom() && (inner.center().x - header.center().x).abs() < 1.0,
        "the second dialog hangs from the first one's header: {inner:?} below {header:?}"
    );
    fixture
        .test
        .snapshot("a_dialog_of_a_programs_dialog_points_at_the_dialog_it_came_from");
}
