use super::*;

#[test]
fn only_thumbnails_are_fetched_for_the_grid() {
    let review = Review::with(Manifest {
        paintings: vec![
            changed("beui-demo.buttons"),
            recording("beui-demo.a_sheet_rises"),
        ],
    });

    assert_eq!(
        review.requested(),
        [
            "small-after-beui-demo.buttons",
            "small-after-beui-demo.a_sheet_rises-2",
            "small-before-beui-demo.buttons",
            "small-before-beui-demo.a_sheet_rises-2",
            "small-changes-beui-demo.buttons",
            "small-changes-beui-demo.a_sheet_rises-2",
        ],
        "the grid fetches what it shows first, then what holding a key shows, and no full image"
    );
}
