use super::*;

#[test]
fn the_manifest_that_failed_says_why() {
    let mut review = Review::open(None);
    review
        .manifest
        .set(Fetch::Failed("HTTP 404 for ../../0123abcd.json".to_owned()));
    review.test.frame(Vec::new());

    assert!(review.test.shown("paint_review.failed"));
    review.test.snapshot("the_manifest_that_failed_says_why");
}
