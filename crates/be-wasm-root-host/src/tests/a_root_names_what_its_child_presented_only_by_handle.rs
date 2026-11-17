use super::*;

#[test]
fn a_root_names_what_its_child_presented_only_by_handle() {
    let mut root = root();
    let done = root
        .frame(FrameRequest { time: PAINTS }, Instant::now() + PATIENT)
        .expect("the root draws");
    assert_eq!(
        shown(&done),
        vec![ROOT_LAYER, PAINTER_LAYER],
        "the frame waited for the painter, so it shows the painter's surface over the root"
    );
    let list = LayerList::decode(&done.layers).expect("the frame is a layer list");
    let painted = list.windows[0].layers[1].surface;
    let Surface::Screen { instance, surface } = painted else {
        panic!("the painter's layer shows a screen, not {painted:?}");
    };
    assert_eq!(surface, PAINTED_SURFACE);

    let presented = root
        .surface(instance, surface)
        .expect("the host holds what the root's child presented, by the handle the root named");
    assert_eq!(presented.surface, PAINTED_SURFACE);
    assert_eq!(
        (presented.texture.width(), presented.texture.height()),
        (4, 4),
        "it is the texture the painter configured, on the host's device"
    );
    assert!(
        root.surface(instance, PAINTED_SURFACE + 1).is_none(),
        "a surface the child never presented names nothing"
    );
    assert!(
        root.surface(instance + 1, PAINTED_SURFACE).is_none(),
        "nor does an instance the root never spawned"
    );
}
