use super::*;

#[test]
fn a_child_that_lets_its_surface_go_names_nothing_there_any_more() {
    let mut root = root();
    let painted = |root: &mut Root| {
        let done = root
            .frame(
                FrameRequest {
                    time: PAINTS,
                    wall_clock: 0,
                },
                Instant::now() + PATIENT,
            )
            .expect("the root draws");
        let list = LayerList::decode(&done.layers).expect("the frame is a layer list");
        match list.windows[0].layers[1].surface {
            Surface::Screen { instance, .. } => instance,
            other => panic!("the painter's layer shows a screen, not {other:?}"),
        }
    };
    let instance = painted(&mut root);
    assert!(root.surface(instance, PAINTED_SURFACE).is_some());

    // Asked again, the painter drops its surface.
    assert_eq!(painted(&mut root), instance, "the root keeps one painter");
    assert!(
        root.surface(instance, PAINTED_SURFACE).is_none(),
        "the host holds no texture of a surface its app let go of"
    );
}
