use super::*;

#[test]
fn a_grip_on_an_area_stays_and_one_off_every_area_comes_to_its_home() {
    let home = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let grip = vec2(96.0, 48.0);
    let at = |x: f32, y: f32| {
        Rect::from_min_size(pos2(x, y), vec2(300.0, 200.0)).grabbable(grip, &[home], home)
    };
    assert_eq!(
        at(800.0 - grip.x + SLACK * 0.8, 10.0),
        pos2(800.0 - grip.x + SLACK * 0.8, 10.0),
        "a grip off its area by less than half a point stays where it is"
    );
    assert_eq!(
        at(800.0 - grip.x + 2.0, 10.0),
        pos2(800.0 - grip.x, 10.0),
        "one off by more comes back until all of it is on"
    );
    assert_eq!(
        at(-40.0, -40.0),
        pos2(0.0, 0.0),
        "one off the top-left corner comes back to it"
    );
    let big = Rect::from_min_size(pos2(30.0, -20.0), vec2(1000.0, 900.0));
    assert_eq!(
        big.grabbable(big.size(), &[home], home),
        home.min,
        "a grip bigger than its home keeps its top-left corner on the home's"
    );
    let beside = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let over = Rect::from_min_size(pos2(1000.0, 100.0), vec2(300.0, 200.0));
    assert_eq!(
        over.grabbable(grip, &[home, beside], home),
        over.min,
        "a grip on another area than its home stays there"
    );
}
