use super::*;

#[test]
fn a_keyboard_step_takes_the_nearest_place_that_way() {
    let rest = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), vec2(20.0, 20.0));
    let rests = [rest(0.0, 0.0), rest(100.0, 0.0), rest(200.0, 10.0), rest(100.0, 100.0)];
    let from = rests[0].center();
    assert_eq!(step_toward(&rests, from, vec2(1.0, 0.0)), Some(1), "the nearest to the right");
    assert_eq!(step_toward(&rests, from, vec2(0.0, 1.0)), Some(3), "only what lies below");
    assert_eq!(step_toward(&rests, from, vec2(-1.0, 0.0)), None, "nothing to the left");
    assert_eq!(
        step_toward(&rests, rests[1].center(), vec2(1.0, 0.0)),
        Some(2),
        "a place a little off the line still counts"
    );
}
