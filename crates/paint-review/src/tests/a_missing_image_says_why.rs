use super::*;

#[test]
fn a_missing_image_says_why() {
    let mut review = Review::with(Manifest {
        paintings: vec![changed("missing")],
    });
    review.deliver();

    assert!(review.test.shown("paint_review.image.failed"));
    review.test.snapshot("a_missing_image_says_why");
}
