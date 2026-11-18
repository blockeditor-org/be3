use be_lib_geometry::pos2;
use be_sdk_root_testing::{ORIGIN, ROOT_LAYER as EMBEDDING_ROOT_LAYER, SCREEN_SIZE};

use super::*;

#[test]
fn a_root_built_on_the_library_shows_the_editor_it_embeds() {
    let mut root = start(EMBEDDING_ROOT);
    let shown_screen = |root: &mut Root, time| {
        let done = root
            .frame(FrameRequest { time }, Instant::now() + PATIENT)
            .expect("the root draws");
        let list = LayerList::decode(&done.layers).expect("the frame is a layer list");
        let [window] = &list.windows[..] else {
            panic!("the root shows one window, not {:?}", list.windows);
        };
        let [own, screen] = &window.layers[..] else {
            panic!(
                "the frame waited for the editor's first frame, so it shows the root's surface \
                 and the editor's screen over it, not {:?}",
                window.layers
            );
        };
        assert_eq!(own.id, EMBEDDING_ROOT_LAYER);
        assert_eq!(own.surface, Surface::Root(0));
        screen.clone()
    };

    let screen = shown_screen(&mut root, 1);
    let Surface::Screen { instance, surface } = screen.surface else {
        panic!("the editor's layer shows a screen, not {:?}", screen.surface);
    };
    assert_eq!(screen.transform.origin(), pos2(ORIGIN.0, ORIGIN.1));
    assert_eq!(screen.size, SCREEN_SIZE);
    let presented = root.surface(instance, surface).expect(
        "the host holds what the triangle presented to its screen, by the handle the root named",
    );
    assert_eq!(
        (presented.texture.width(), presented.texture.height()),
        SCREEN_SIZE,
        "the triangle drew its screen at the size the root asked for, on the host's device"
    );

    assert_eq!(
        shown_screen(&mut root, 2),
        screen,
        "with no frame due, the next frame shows the screen as it was"
    );
}
