use super::*;

#[test]
fn the_image_can_return_to_fit_after_zooming() {
    let mut big = changed("sample");
    let frame = &mut big.frames[0];
    for picture in [&mut frame.before, &mut frame.after].into_iter().flatten() {
        picture.width = 1200;
        picture.height = 800;
    }
    let mut review = Review::open(Some("sample"));
    review.load(Manifest {
        paintings: vec![big],
    });
    for request in review.requests.borrow_mut().drain(..) {
        request.deliver(Ok(Image::from_rgba(1200, 800, vec![120; 1200 * 800 * 4])));
    }
    review.test.frame(Vec::new());
    review.test.settle();
    assert!(review.camera().scale < 1.0, "a large painting opens fitted");
    review.test.click("paint_review.actual_size");
    review.test.settle();
    assert_eq!(review.camera().scale, 1.0);
    review.test.snapshot("the_images_at_actual_size");
    review.test.click("paint_review.fit");
    review.test.settle();
    assert!(review.camera().scale < 1.0);
    review.test.snapshot("the_images_fit_the_stage");
}
