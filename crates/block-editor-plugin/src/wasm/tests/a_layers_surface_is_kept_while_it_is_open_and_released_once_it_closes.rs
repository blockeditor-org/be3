use super::*;

#[test]
fn a_layers_surface_is_kept_while_it_is_open_and_released_once_it_closes() {
    let mut rig = Rig::new(Some(space()));
    let small = Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.0));
    rig.open(vec![layer(1, 0, small)]);
    let first = Rig::layers(&rig.draw())[0].surface;

    rig.open(vec![layer(1, 0, small.translate(vec2(5.0, 0.0)))]);
    let moved = rig.draw();
    let moved = Rig::layers(&moved);
    assert_eq!(moved[0].surface, first, "a moved layer keeps its surface");

    rig.open(vec![layer(
        1,
        0,
        Rect::from_min_size(small.min, vec2(90.0, 40.0)),
    )]);
    let grown = Rig::layers(&rig.draw());
    assert_eq!(grown[0].surface, first, "a resized layer keeps its surface");
    assert_eq!(
        rig.shared.seen.borrow().painted.last(),
        Some(&(ViewId(1), [90, 80])),
        "and is painted at its new size"
    );
    assert_eq!(rig.surfaces.layer_surfaces(), [first]);

    rig.open(vec![layer(2, 0, small)]);
    let replaced = Rig::layers(&rig.draw());
    assert_ne!(
        replaced[0].surface, first,
        "a newly opened layer gets a surface of its own"
    );
    assert_eq!(
        rig.surfaces.layer_surfaces(),
        [replaced[0].surface],
        "the closed layer's surface was released"
    );

    rig.open(Vec::new());
    let closed = rig.draw();
    assert!(
        Rig::layers(&closed).is_empty(),
        "the report lists no layers"
    );
    assert_eq!(rig.surfaces.layer_surfaces(), Vec::<u32>::new());
}
