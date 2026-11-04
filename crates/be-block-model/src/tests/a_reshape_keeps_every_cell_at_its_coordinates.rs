use super::*;

#[test]
fn a_reshape_keeps_every_cell_at_its_coordinates() {
    let mut document = painted(&picture(Bounds::new(0, 0, 3, 3)), &[(0, 0, 1), (2, 2, 9)]);

    document.apply(
        &Picture::PIXELS
            .reshape(ObjectId::ROOT, Bounds::new(-2, -2, 5, 5))
            .into(),
    );
    assert_eq!(pixel(&document, 0, 0), Some(1));
    assert_eq!(pixel(&document, -2, -2), Some(0));
    assert_eq!(pixel(&document, 2, 2), Some(9));
    assert_eq!(pixel(&document, 3, 3), None);

    document.apply(
        &Picture::PIXELS
            .reshape(ObjectId::ROOT, Bounds::new(1, 1, 2, 2))
            .into(),
    );
    assert_eq!(pixel(&document, 0, 0), None);
    assert_eq!(pixel(&document, 2, 2), Some(9));
    assert_eq!(document.root().pixels.bounds(), Bounds::new(1, 1, 2, 2));
}
