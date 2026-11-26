use super::*;

#[test]
fn dragging_a_trim_handle_lengthens_its_clip_by_how_far_it_moved() {
    let (mut fixture, clip) = editor_with_clip();

    let body = fixture.editor.rect_of(&format!("video.clip.{clip}"));
    let scale = body.width() / 30.0;
    let handle = fixture.editor.rect_of(&format!("video.trim.{clip}"));
    let moved = 20.0 * scale;
    fixture.editor.drag(
        handle.center(),
        Pos2::new(handle.center().x + moved, handle.center().y),
    );

    let length = fixture.video().clips()[0].length;
    assert!(
        length.abs_diff(50) <= 1,
        "a handle dragged twenty frames' worth of pixels adds twenty frames, not {length}"
    );
}
