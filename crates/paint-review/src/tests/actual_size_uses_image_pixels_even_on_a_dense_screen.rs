use super::*;

#[test]
fn actual_size_uses_image_pixels_even_on_a_dense_screen() {
    let mut review = Review::sized(Some("sample"), Vec2::new(390.0, 844.0));
    review.test.context().set_pixels_per_point(2.0);
    review.load(Manifest {
        paintings: vec![changed("sample")],
    });
    review.deliver();
    review.test.settle();
    review.test.click("paint_review.actual_size");
    review.test.settle();
    let document = review.test.document();
    let camera = beui::unstyled::pan_zoom_view(
        document,
        document.find_test_id("paint_review.pan_zoom").unwrap(),
    )
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
    review.test.frame(vec![
        beui::Event::PointerMoved(stage.center()),
        beui::Event::Zoom(2.0),
    ]);
    review.test.settle();
    let document = review.test.document();
    let zoomed = beui::unstyled::pan_zoom_view(
        document,
        document.find_test_id("paint_review.pan_zoom").unwrap(),
    )
    .get();
    assert_eq!(zoomed.scale, 2.0);
    review.test.click("paint_review.actual_size");
    review.test.settle();
    let document = review.test.document();
    assert_eq!(
        beui::unstyled::pan_zoom_view(
            document,
            document.find_test_id("paint_review.pan_zoom").unwrap()
        )
        .get()
        .scale,
        1.0
    );
}
