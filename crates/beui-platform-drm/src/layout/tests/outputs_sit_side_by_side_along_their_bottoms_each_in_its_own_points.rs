use super::*;

#[test]
fn outputs_sit_side_by_side_along_their_bottoms_each_in_its_own_points() {
    let outputs = arrange(&[((2560, 1440), 2.0), ((1920, 1080), 1.0)]);

    assert_eq!(
        outputs,
        [
            Rect::from_min_size(pos2(0.0, 360.0), vec2(1280.0, 720.0)),
            Rect::from_min_size(pos2(1280.0, 0.0), vec2(1920.0, 1080.0)),
        ]
    );
    assert_eq!(
        bounds(&outputs),
        Rect::from_min_size(Pos2::ZERO, vec2(3200.0, 1080.0))
    );
}
