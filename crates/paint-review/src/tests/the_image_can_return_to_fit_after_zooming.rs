use super::*;

#[test]
fn the_image_can_return_to_fit_after_zooming() {
    let mut review = Review::with(Manifest {
        paintings: vec![changed("sample")],
    });
    for request in review.requests.borrow_mut().drain(..) {
        request.deliver(Ok(Image::from_rgba(1200, 800, vec![120; 1200 * 800 * 4])));
    }
    review.test.frame(Vec::new());
    review.test.click("paint_review.mode.side_by_side");
    review.test.click("paint_review.actual_size");
    review.test.settle();
    let camera = || {
        let document = review.test.document();
        document
            .api::<beui::styled::PanZoomApi>(
                document.find_test_id("paint_review.pan_zoom").unwrap(),
            )
            .view()
            .get()
    };
    assert_eq!(camera().scale, 1.0);
    review.test.snapshot("the_images_at_actual_size");
    review.test.click("paint_review.fit");
    review.test.settle();
    let document = review.test.document();
    let fitted = document
        .api::<beui::styled::PanZoomApi>(document.find_test_id("paint_review.pan_zoom").unwrap())
        .view()
        .get();
    assert!(fitted.scale < 1.0);
    review.test.snapshot("the_images_fit_the_stage");
}
