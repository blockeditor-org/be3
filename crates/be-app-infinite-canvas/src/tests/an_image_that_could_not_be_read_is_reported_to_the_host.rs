use super::*;
use be_sdk_test::HostCall;

#[test]
fn an_image_that_could_not_be_read_is_reported_to_the_host() {
    let mut editor = editor(&[]);
    editor.click(editor.rect_of("infinite-canvas.canvas").center());
    editor.take_requests();

    editor.run_action("canvas.add.image");
    let reply = editor
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::PickFile { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("adding an image asks the host for a file");
    editor.answer(reply, Err("The file could not be read.".to_owned()));
    editor.run();

    assert_eq!(
        editor.take_reported_failures(),
        ["The image could not be added. The file could not be read."],
        "the canvas has the host say why"
    );
    assert!(entities(&editor).is_empty(), "nothing is added");
}
