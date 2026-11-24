use be_sdk_api::Occluder;

use super::*;

#[test]
fn a_placers_occluders_take_the_presses_over_its_children() {
    // A live child at the placer's left, with its menu's occluder over the child's top.
    let hole = rect((0.0, 0.0), (40.0, 80.0));
    let menu = rect((0.0, 0.0), (60.0, 20.0));
    let Area::Region(rects) = placer_hit((100, 80), vec![(hole, vec![menu])]) else {
        panic!("the placer is hit around its child and over its occluder");
    };
    let hits = |at: (f32, f32)| rects.iter().any(|rect| rect.contains(pos2(at.0, at.1)));
    assert!(hits((10.0, 10.0)), "over the occluder");
    assert!(hits((70.0, 40.0)), "beside the child");
    assert!(!hits((10.0, 40.0)), "in the child, outside the occluder");

    // Two children side by side, and an occluder over both placed after only the first: it
    // hides the first, so the placer takes the presses over it there, but not the second,
    // which keeps them, as be-host-runtime says each hole with its own occluders.
    let first = rect((0.0, 0.0), (40.0, 80.0));
    let second = rect((50.0, 0.0), (90.0, 80.0));
    let band = rect((0.0, 0.0), (100.0, 20.0));
    let Area::Region(rects) =
        placer_hit((100, 80), vec![(first, vec![band]), (second, Vec::new())])
    else {
        panic!("the placer is hit around its children");
    };
    let hits = |at: (f32, f32)| rects.iter().any(|rect| rect.contains(pos2(at.0, at.1)));
    assert!(
        hits((10.0, 10.0)),
        "over the first child, under the occluder hiding it"
    );
    assert!(
        !hits((60.0, 10.0)),
        "over the second child, which it does not hide"
    );
    assert!(
        !hits((10.0, 40.0)) && !hits((60.0, 40.0)),
        "below the band, in each child"
    );

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
        children: vec![
            placed(first, ChildMode::Live),
            placed(second, ChildMode::Live),
        ],
        occluders: vec![Occluder {
            after: 1,
            rect: band,
        }],
    };
    assert_eq!(occluders_of(&placements, 0), vec![band]);
    assert!(occluders_of(&placements, 1).is_empty());
    placements.occluders[0].after = 0;
    assert!(occluders_of(&placements, 0).is_empty());
}
