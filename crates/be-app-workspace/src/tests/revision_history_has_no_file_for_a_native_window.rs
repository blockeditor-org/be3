use super::*;

#[test]
fn revision_history_has_no_file_for_a_native_window() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 720.0)));
    show(&mut fixture, opened, None);
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![window(7, "Terminal", None)]);
    fixture.settle();
    fixture.show_history();
    assert!(fixture.says("Open a file to see its revision history."));
    fixture.click_text("Untitled");
    fixture.show_history();
    assert!(!fixture.says("Open a file to see its revision history."));
    fixture.close_active_tab();
    assert!(fixture.says("Open a file to see its revision history."));
}
