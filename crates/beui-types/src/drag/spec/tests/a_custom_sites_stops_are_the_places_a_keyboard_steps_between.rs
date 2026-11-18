use super::*;

/// A `custom` site's `stops` are listed as its places, in its node's own points mapped to
/// where it is, all under the site's one id, so a keyboard step aims at each in turn and the
/// site's `place` answers what dropping there means.
#[test]
fn a_custom_sites_stops_are_the_places_a_keyboard_steps_between() {
    let root = RootScope::new();
    let node = origin(&root);
    let spec = custom(node, |at: Pos2, _| {
        (at.x < 100.0).then(|| (at.x / 10.0) as u32)
    })
    .stepping(|size: Vec2| {
        assert_eq!(
            size,
            vec2(500.0, 250.0),
            "given its node's size in its own points"
        );
        (0..3)
            .map(|frame| {
                Stop::new(
                    Rect::from_center_size(pos2(frame as f32 * 10.0 + 5.0, 5.0), vec2(10.0, 10.0)),
                    format!("frame {frame}"),
                )
            })
            .collect()
    });
    // The node at (100, 100), drawn at twice its size.
    let placed = |_: NodeId| {
        Some((
            Rect::from_min_size(pos2(100.0, 100.0), vec2(1000.0, 500.0)),
            2.0,
        ))
    };
    let rests = rests(&spec, &placed);

    assert_eq!(rests.len(), 3);
    assert!(
        rests.iter().all(|rest| rest.id == rests[0].id),
        "every stop is the site's one place"
    );
    assert_eq!(rests[1].label.as_deref(), Some("frame 1"));
    assert_eq!(
        rests[1].aim(),
        pos2(130.0, 110.0),
        "aimed at the stop's middle, where the node is"
    );
    let over = frame(&spec, at(pos2(15.0, 5.0)), &DragFrame::default());
    assert_eq!(
        over.drop,
        Some(1),
        "and the place answers for the point it aims at"
    );
}
