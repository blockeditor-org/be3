use super::*;

#[test]
fn a_recording_is_reviewed_one_frame_at_a_time() {
    let (review, mut editor) = Review::open();
    review.write(PATH, &recording(&[20, 130, 240]));
    editor.click("paint_review.refresh");
    editor.run();
    editor.run();
    editor.click(&entry_id(PATH));
    settled(&mut editor);
    assert_eq!(shown_frame(&editor), 0);
    assert_eq!(rasters(&editor), 3);
    editor.record();

    editor.click("paint_review.frame.next");
    settled(&mut editor);
    assert_eq!(shown_frame(&editor), 1);
    assert_eq!(rasters(&editor), 3);
    editor.record();

    editor.click("paint_review.frame.next");
    settled(&mut editor);
    editor.click("paint_review.frame.previous");
    settled(&mut editor);
    assert_eq!(shown_frame(&editor), 1);
    editor.record();

    editor.click("paint_review.frame.play");
    editor.run();
    let started = shown_frame(&editor);
    editor.run();
    editor.run();
    assert_ne!(shown_frame(&editor), started);
    editor.record();

    editor.click("paint_review.frame.play");
    editor.run();
    let paused = shown_frame(&editor);
    editor.run();
    editor.run();
    assert_eq!(shown_frame(&editor), paused);

    editor.snapshot("stepping_through_a_recording");
}
