use super::*;

#[test]
fn actual_size_uses_image_pixels_even_on_a_dense_screen() {
    let mut review = Review::sized(Some("sample"), Vec2::new(390.0, 844.0));
    review.test.rescale(2.0);
    review.load(Manifest {
        paintings: vec![changed("sample")],
    });
    review.deliver();
    review.test.settle();
    review.test.click("paint_review.actual_size");
    review.test.settle();
    let document = review.test.document();
    let camera = document
        .api::<beui::styled::PanZoomApi>(document.find_test_id("paint_review.pan_zoom").unwrap())
        .view()
        .get();
    assert_eq!(camera.scale, 1.0);
    assert_eq!(
        camera.center,
        beui::pos2(WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0)
    );
    review
        .test
        .snapshot("actual_size_uses_image_pixels_even_on_a_dense_screen");
    let stage = review.test.rect_of("paint_review.pan_zoom");
    review
        .test
        .frame(vec![Input::PointerMoved(stage.center()), Input::Zoom(2.0)]);
    review.test.settle();
    let document = review.test.document();
    let zoomed = document
        .api::<beui::styled::PanZoomApi>(document.find_test_id("paint_review.pan_zoom").unwrap())
        .view()
        .get();
    assert_eq!(zoomed.scale, 2.0);
    review.test.click("paint_review.actual_size");
    review.test.settle();
    let document = review.test.document();
    assert_eq!(
        document
            .api::<beui::styled::PanZoomApi>(
                document.find_test_id("paint_review.pan_zoom").unwrap()
            )
            .view()
            .get()
            .scale,
        1.0
    );
}
