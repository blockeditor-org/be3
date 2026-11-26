use super::*;

#[test]
fn a_transform_and_its_inverse_take_a_point_there_and_back() {
    let turned = Affine::from_axes(vec2(0.0, 2.0), vec2(-2.0, 0.0), pos2(30.0, 40.0));
    let point = pos2(5.0, 7.0);
    assert!(close(turned.apply(point), pos2(16.0, 50.0)));
    let back = turned.inverse().expect("a turn and a scale invert");
    assert!(close(back.apply(turned.apply(point)), point));
    let both = turned.then(&Affine::scaling(0.5));
    assert!(close(both.apply(point), pos2(8.0, 25.0)));
    assert_eq!(
        Affine::from_axes(vec2(1.0, 1.0), vec2(2.0, 2.0), Pos2::ZERO).inverse(),
        None,
        "a transform that flattens the surface has no inverse"
    );
}
