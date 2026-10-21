use super::*;

#[test]
fn only_the_frame_shown_is_fetched() {
    let review = Review::with(Manifest {
        paintings: vec![
            recording("beui-demo.a_sheet_rises"),
            changed("beui-demo.buttons"),
        ],
    });

    let mut requested = review.requested();
    requested.sort();
    assert_eq!(
        requested,
        [
            "after-beui-demo.a_sheet_rises-2",
            "before-beui-demo.a_sheet_rises-2",
            "changes-beui-demo.a_sheet_rises-2",
        ],
        "the frame shown is fetched whole, so flipping it is instant, and nothing else is"
    );
}
