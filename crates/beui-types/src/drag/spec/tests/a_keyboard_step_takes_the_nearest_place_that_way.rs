use super::*;

#[test]
fn a_keyboard_step_takes_the_nearest_place_that_way() {
    let rest = |x: f32, y: f32| (Rect::from_min_size(pos2(x, y), vec2(20.0, 20.0)), None);
    let rests = [
        rest(0.0, 0.0),
        rest(100.0, 0.0),
        rest(200.0, 10.0),
        rest(100.0, 100.0),
    ];
    let from = rests[0].0.center();
    assert_eq!(
        step_toward(&rests, from, vec2(1.0, 0.0)),
        Some(1),
        "the nearest to the right"
    );
    assert_eq!(
        step_toward(&rests, from, vec2(0.0, 1.0)),
        Some(3),
        "only what lies below"
    );
    assert_eq!(
        step_toward(&rests, from, vec2(-1.0, 0.0)),
        None,
        "nothing to the left"
    );
    assert_eq!(
        step_toward(&rests, rests[1].0.center(), vec2(1.0, 0.0)),
        Some(2),
        "a place a little off the line still counts"
    );
    let edge = |x: f32, y: f32, side: Side| {
        (Rect::from_min_size(pos2(x, y), vec2(20.0, 20.0)), Some(side))
    };
    let panes = [edge(60.0, 10.0, Side::Above), edge(300.0, 200.0, Side::Right)];
    assert_eq!(
        step_toward(&panes, pos2(10.0, 10.0), vec2(1.0, 0.0)),
        Some(1),
        "an edge's place answers only the arrow it faces, however near another edge's is"
    );
    assert_eq!(
        step_toward(&panes, pos2(70.0, 100.0), vec2(0.0, -1.0)),
        Some(0)
    );
}
