use super::*;

#[test]
fn the_editor_reports_the_canvas_as_the_content_the_host_pans() {
    let (mut editor, host) = editor();

    editor.run();

    let canvas = canvas(&mut editor);
    assert!(canvas.left() > editor.rect().left());
    assert_eq!(host.take_beui_content(), Some(canvas));
}
