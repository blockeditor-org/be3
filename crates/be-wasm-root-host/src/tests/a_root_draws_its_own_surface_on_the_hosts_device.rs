use be_sdk_root_testing::{
    RESIZED, RESIZES, ROOT_LAYER as EMBEDDING_ROOT_LAYER, ROOT_SURFACE, SIZE, UNPRESENTED_SURFACE,
};

use super::*;

#[test]
fn a_root_draws_its_own_surface_on_the_hosts_device() {
    let mut root = start(EMBEDDING_ROOT);
    assert!(
        root.own_surface(ROOT_SURFACE).is_none(),
        "the root has drawn nothing of its own before its first frame"
    );
    let done = root
        .frame(FrameRequest { time: 1 }, Instant::now() + PATIENT)
        .expect("the root draws");
    let list = LayerList::decode(&done.layers).expect("the frame is a layer list");
    let own = &list.windows[0].layers[0];
    assert_eq!(own.id, EMBEDDING_ROOT_LAYER);
    assert_eq!(
        own.surface,
        Surface::Root(ROOT_SURFACE),
        "the root names its own surface by the id it drew it with"
    );
    let drawn = root
        .own_surface(ROOT_SURFACE)
        .expect("the host holds what the root presented to its own surface")
        .clone();
    assert_eq!(
        (drawn.texture.width(), drawn.texture.height()),
        SIZE,
        "the root drew its surface at the size it asked for, on the host's device"
    );
    assert_eq!(own.size, SIZE, "its layer shows the whole of it");

    root.frame(FrameRequest { time: 2 }, Instant::now() + PATIENT)
        .expect("the root draws");
    let kept = root
        .own_surface(ROOT_SURFACE)
        .expect("the surface still shows what the root presented");
    assert_eq!(
        kept.generation, drawn.generation,
        "a root that presents nothing new shows the same texture, so its host draws nothing anew"
    );
    assert!(
        root.own_surface(1).is_none(),
        "a surface the root never presented resolves to nothing, though its layer list names it \
         when its frame is late"
    );

    root.frame(FrameRequest { time: RESIZES }, Instant::now() + PATIENT)
        .expect("the root draws");
    let resized = root
        .own_surface(ROOT_SURFACE)
        .expect("a surface configured afresh still shows what was presented to it");
    assert_eq!(
        (
            resized.texture.width(),
            resized.texture.height(),
            resized.generation
        ),
        (SIZE.0, SIZE.1, drawn.generation),
        "until the root presents it at its new size {RESIZED:?}, it shows its last frame"
    );
    assert!(
        root.own_surface(UNPRESENTED_SURFACE).is_none(),
        "a surface configured but never presented shows nothing"
    );
}
