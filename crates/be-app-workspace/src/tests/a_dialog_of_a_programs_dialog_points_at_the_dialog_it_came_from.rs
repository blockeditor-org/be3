use super::*;

#[test]
fn a_dialog_of_a_programs_dialog_points_at_the_dialog_it_came_from() {
    let (mut fixture, _) = editor();
    let confirm = TestWindow {
        modal: true,
        size: be_sdk_beui::beui::Vec2::new(240.0, 120.0),
        ..window(3, "Replace the file?", Some(2))
    };
    fixture.show_windows(&[
        window(1, "Text Editor", None),
        window(2, "Save as", Some(1)),
        confirm,
    ]);
    fixture.settle();

    let header = {
        let document = fixture.test.document();
        let node = document
            .find_test_id(&format!("dock.dialog.{}.header", (1u64 << 41) + 2))
            .expect("the first dialog has a header");
        document
            .node_rect(node)
            .expect("the first dialog's header is laid out beneath the second")
    };
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
