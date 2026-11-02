use super::*;

#[test]
fn a_recording_steps_through_the_frames_that_changed() {
    let mut review = Review::with(Manifest {
        paintings: vec![recording("beui-demo.a_sheet_rises")],
    });
    review.deliver();

    review.test.click("paint_review.frame.next");
    review.test.frame(Vec::new());
    review.deliver();

    review.press(beui::Key::Period);
    review.deliver();

    assert_eq!(
        review.requested(),
        Vec::<String>::new(),
        "every frame shown was fetched"
    );
    review
        .test
        .snapshot("a_recording_steps_through_the_frames_that_changed");
}
