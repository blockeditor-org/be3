use super::*;

#[test]
fn outputs_sit_side_by_side_in_points() {
    let outputs = arrange(&[(2560, 1440), (1920, 1080)], 2.0);

    assert_eq!(
        outputs,
        [
            Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0)),
            Rect::from_min_size(pos2(1280.0, 0.0), vec2(960.0, 540.0)),
        ]
    );
    assert_eq!(
        bounds(&outputs),
        Rect::from_min_size(Pos2::ZERO, vec2(2240.0, 720.0))
    );
}
