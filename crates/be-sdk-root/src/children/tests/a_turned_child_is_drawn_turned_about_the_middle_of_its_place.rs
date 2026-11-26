use be_lib_geometry::Pos2;

use super::*;

#[test]
fn a_turned_child_is_drawn_turned_about_the_middle_of_its_place() {
    let mut placement = placed(rect((100.0, 100.0), (300.0, 200.0)), ChildMode::Passive);
    placement.rotation = std::f32::consts::FRAC_PI_2;
    let to_placer = placed_transform(&placement, be_lib_geometry::vec2(1.0, 1.0));
    let near = |at: Pos2, to: Pos2| (at - to).length() < 0.001;
    // Its middle stays where it was placed; a quarter turn takes its top left to its top right.
    assert!(near(to_placer.apply(pos2(100.0, 50.0)), pos2(200.0, 150.0)));
    assert!(near(to_placer.apply(pos2(0.0, 0.0)), pos2(250.0, 50.0)));
    // What shows of it is the bounds of its place turned.
    let swept = to_placer.bounds(Rect::from_min_size(pos2(0.0, 0.0), placement.rect.size()));
    assert!(near(swept.min, pos2(150.0, 50.0)) && near(swept.max, pos2(250.0, 250.0)));
}
