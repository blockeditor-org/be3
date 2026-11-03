use super::*;

#[test]
fn hovering_fetches_the_paintings_nearby() {
    let paintings: Vec<Painting> = (0..30).map(|n| changed(&format!("p{n:02}"))).collect();
    let mut review = Review::with(Manifest { paintings });
    review.deliver();
    let top = review.test.rect_of("paint_review.painting.p00").min.y;
    let mut columns = 1;
    while review
        .test
        .rect_of(&format!("paint_review.painting.p{columns:02}"))
        .min
        .y
        == top
    {
        columns += 1;
    }
    let middle = columns + columns / 2;

    review
        .test
        .hover(format!("paint_review.painting.p{middle:02}").as_str());
    let requested = review.requested();
    for near in [middle - 1, middle + 1, middle - columns, middle + columns] {
        assert!(
            requested.contains(&format!("after-p{near:02}")),
            "p{near:02} is next to p{middle:02} in a grid of {columns} columns: {requested:?}"
        );
    }
    assert!(!requested.contains(&format!("after-p{:02}", middle + 2 * columns)));
    assert!(requested.contains(&format!("before-p{middle:02}")));
    assert!(requested.contains(&format!("changes-p{middle:02}")));
}
