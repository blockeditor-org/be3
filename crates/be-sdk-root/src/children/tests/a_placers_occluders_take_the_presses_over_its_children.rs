use be_sdk_api::Occluder;

use super::*;

#[test]
fn a_placers_occluders_take_the_presses_over_its_children() {
    // A live child at the placer's left, with its menu's occluder over the child's top.
    let hole = rect((0.0, 0.0), (40.0, 80.0));
    let menu = rect((0.0, 0.0), (60.0, 20.0));
    let Area::Region(rects) = placer_hit((100, 80), vec![hole], vec![menu]) else {
        panic!("the placer is hit around its child and over its occluder");
    };
    assert!(rects.contains(&menu));
    assert!(rects.contains(&rect((40.0, 0.0), (100.0, 80.0))));

    // The child is hit everywhere but under the occluder.
    let Area::Region(rects) = without((40, 80), &[rect((0.0, 0.0), (40.0, 20.0))]) else {
        panic!("the child is hit only outside the occluder");
    };
    assert_eq!(rects, vec![rect((0.0, 20.0), (40.0, 80.0))]);
    assert_eq!(without((40, 80), &[]), Area::Whole);

    // An occluder hides only the children placed before it.
    let mut placements = ChildPlacements {
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        children: vec![placed(hole, ChildMode::Live)],
        occluders: vec![Occluder {
            after: 1,
            rect: menu,
        }],
    };
    let hidden = occluders_of(&placements, 0);
    assert_eq!(hidden, vec![menu]);
    placements.occluders[0].after = 0;
    assert!(occluders_of(&placements, 0).is_empty());
}
