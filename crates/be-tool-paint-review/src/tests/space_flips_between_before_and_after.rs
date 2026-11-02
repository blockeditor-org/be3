use super::*;

#[test]
fn space_flips_between_before_and_after() {
    let mut review = Review::with(mixed());
    review.deliver();
    assert!(review.pressed("after"));

    review.press(beui::Key::Space);
    assert!(
        review.pressed("before"),
        "space shows what the painting was"
    );
    review.press(beui::Key::Space);
    assert!(review.pressed("after"), "and space again what it became");

    review.press(beui::Key::Three);
    assert!(review.pressed("changes"));
    review.press(beui::Key::Space);
    assert!(
        review.pressed("before"),
        "from the changes, space flips to before"
    );
    assert_eq!(
        review.requested(),
        Vec::<String>::new(),
        "flipping needs nothing it did not already fetch"
    );
}
