use super::*;

#[test]
fn painting_outside_a_grid_or_with_the_wrong_cell_size_changes_nothing() {
    let document = picture(Bounds::new(0, 0, 2, 2));

    let outside = painted(&document, &[(2, 0, 1), (-1, 0, 1), (0, 2, 1)]);
    let mut wrong_size = document.clone();
    wrong_size.apply(
        &Change::Paint {
            object: ObjectId::ROOT,
            field: Picture::PIXELS.index(),
            cells: vec![Paint {
                x: 0,
                y: 0,
                expected: None,
                value: vec![1, 2],
            }],
        }
        .into(),
    );

    assert_eq!(outside, document);
    assert_eq!(wrong_size, document);
    assert_eq!(
        document.step(&Picture::PIXELS.paint(ObjectId::ROOT, [(5, 5, [1])]).into()),
        None
    );
}
