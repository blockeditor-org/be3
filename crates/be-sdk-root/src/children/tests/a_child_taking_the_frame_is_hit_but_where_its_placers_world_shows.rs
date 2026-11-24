use be_sdk_api::Insets;

use super::*;

#[test]
fn a_child_taking_the_frame_is_hit_but_where_its_placers_world_shows() {
    // A 200 by 100 point frame at 2 pixels a point, its content below a 20 point bar, the
    // child placed at 50,40 to 100,80 within it.
    let content = Insets {
        left: 0.0,
        top: 20.0,
        right: 0.0,
        bottom: 0.0,
    };
    let placed = rect((50.0, 40.0), (100.0, 80.0));
    let Area::Region(hit) = framing_hit((400, 200), 2.0, Some(content), placed) else {
        panic!("a child that says where its content is is hit only in part");
    };
    let hits = |x: f32, y: f32| hit.iter().any(|rect| rect.contains(pos2(x, y)));
    // Its bar and its placed content are its own.
    assert!(hits(10.0, 10.0));
    assert!(hits(150.0, 100.0));
    // The rest of its content is where its placer shows, whose press takes the frame back.
    assert!(!hits(10.0, 100.0));
    assert!(!hits(300.0, 180.0));

    // Before its view says where its content is, all of it is.
    assert_eq!(framing_hit((400, 200), 2.0, None, placed), Area::Whole);
}
