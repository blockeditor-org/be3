use super::*;

#[test]
fn the_editor_reports_the_canvas_as_the_content_the_host_pans() {
    let (mut test, editor) = editor();

    test.run();

    let canvas = editor.content_rect();
    assert!(canvas.left() > test.rect().left());
    assert_eq!(editor.host().take_beui_content(), Some(canvas));
}
