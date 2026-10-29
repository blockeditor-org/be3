use super::*;

#[test]
fn a_layer_is_committed_with_its_surface_in_the_frame_that_opens_it() {
    let mut rig = Rig::new(Some(space()));
    let menu = Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.5));
    let submenu = Rect::from_min_size(pos2(280.0, 140.0), vec2(50.0, 30.0));
    rig.open(vec![layer(4, 0, menu), layer(5, 4, submenu)]);
    let commit = rig.draw();

    assert_eq!(
        rig.shared.seen.borrow().spaces,
        [Some(space())],
        "the frame was laid out against the layer space its screen was requested with"
    );
    let layers = Rig::layers(&commit);
    assert_eq!(
        layers.iter().map(|layer| layer.view).collect::<Vec<_>>(),
        [ViewId(4), ViewId(5)],
        "both layers are in the frame that opened them, in the order they opened"
    );
    let surfaces: Vec<u32> = layers.iter().map(|layer| layer.surface).collect();
    assert!(
        surfaces[0] != surfaces[1] && !surfaces.contains(&rig.screen_surface()),
        "each layer has a surface of its own: {surfaces:?}"
    );
    assert_eq!(layers[0].rect, menu);
    assert_eq!(
        rig.shared.seen.borrow().painted,
        [(ViewId(4), [60, 81]), (ViewId(5), [50, 60])],
        "each layer was painted at the raster over its zoom on each axis, in the frame that opened it"
    );
    assert!(
        commit.presented.is_some(),
        "the frame presented its layers with the screen"
    );
}
