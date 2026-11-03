use super::*;

#[test]
fn a_dialog_of_a_programs_dialog_points_at_the_dialog_it_came_from() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let save = TestWindow {
        title: "Save as".to_owned(),
        parent: Some(HostWindowId(1)),
        ..host_window(2, false)
    };
    let confirm = TestWindow {
        title: "Replace the file?".to_owned(),
        parent: Some(HostWindowId(2)),
        modal: true,
        size: Vec2::new(240.0, 120.0),
        ..host_window(3, false)
    };
    fixture.show_windows(&[host_window(1, false), save, confirm]);

    let header = {
        let document = fixture.test.document();
        let node = document
            .find_test_id(&format!("dock.dialog.{}.header", dialog_tab(2)))
            .expect("the first dialog has a header");
        document
            .node_rect(node)
            .expect("the first dialog's header is laid out beneath the second")
    };
    let inner = fixture
        .test
        .rect_of(&format!("dock.dialog.{}.header", dialog_tab(3)));
    assert!(
        inner.top() > header.bottom() && (inner.center().x - header.center().x).abs() < 1.0,
        "the second dialog hangs from the first one's header: {inner:?} below {header:?}"
    );
    fixture
        .test
        .snapshot("a_dialog_of_a_programs_dialog_points_at_the_dialog_it_came_from");
}
