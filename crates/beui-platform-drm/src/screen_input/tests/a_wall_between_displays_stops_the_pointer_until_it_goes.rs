use super::*;

#[test]
fn a_wall_between_displays_stops_the_pointer_until_it_goes() {
    let outputs = [display(), below()];
    let mut input = input(vec![bottom()], Vec::new());

    let at = input.moved(pos2(400.0, 590.0), vec2(0.0, 50.0), &outputs);
    assert_eq!(
        at,
        pos2(400.0, 599.0),
        "a wall stops the pointer short of the display below"
    );

    input.set(Vec::new(), Vec::new());
    let at = input.moved(at, vec2(0.0, 50.0), &outputs);
    assert_eq!(at, pos2(400.0, 649.0), "without it, the pointer crosses");
}
