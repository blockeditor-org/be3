use super::*;

/// What a surface showed stays after its app let it go while its host keeps it
/// (`Root::retain`), as be-host keeps what the root's layer list still names, as be-sdk-root names
/// the last frame of a view hidden while its app draws it anew (`Embedded::kept_layers`), and
/// goes with the first frame whose list does not.
#[test]
fn a_surface_a_child_let_go_of_shows_only_while_the_list_names_it() {
    let mut root = root();
    let drawn = |root: &mut Root, time: u64| {
        let done = root
            .frame(
                FrameRequest {
                    time,
                    wall_clock: 0,
                    utc_offset: 0,
                    preferences: Default::default(),
                    window: window(),
                    describe: false,
                    phone: false,
                },
                Instant::now() + PATIENT,
            )
            .expect("the root draws");
        let list = LayerList::decode(&done.layers).expect("the frame is a layer list");
        // As be-host keeps what the list names.
        let named: Vec<(u64, u32)> = list.windows[0]
            .layers
            .iter()
            .filter_map(|layer| match layer.surface {
                Surface::Screen { instance, surface } => Some((instance, surface)),
                _ => None,
            })
            .collect();
        root.retain(|instance, surface| named.contains(&(instance, surface)));
        list
    };
    let painted = |list: &LayerList| match list.windows[0].layers[1].surface {
        Surface::Screen { instance, .. } => instance,
        other => panic!("the painter's layer shows a screen, not {other:?}"),
    };
    let instance = painted(&drawn(&mut root, PAINTS));
    assert!(root.surface(instance, PAINTED_SURFACE).is_some());

    // Asked again, the painter drops its surface, which the root's list still names.
    assert_eq!(
        painted(&drawn(&mut root, PAINTS)),
        instance,
        "the root keeps one painter"
    );
    assert!(
        root.surface(instance, PAINTED_SURFACE).is_some(),
        "the host keeps what a surface its app let go of showed while the root names it"
    );

    // A frame that shows no painter names its surface no longer.
    let list = drawn(&mut root, 0);
    assert!(
        list.windows[0].layers.iter().all(|layer| layer.id != PAINTER_LAYER),
        "the root shows the painter only in a `PAINTS` frame"
    );
    assert!(
        root.surface(instance, PAINTED_SURFACE).is_none(),
        "the host holds no texture of a surface its app let go of once the root names it no more"
    );
}
