use be_lib_geometry::pos2;
use be_lib_layers::Layer;
use be_sdk_root_testing::{
    CLOSES, ORIGIN, RASTER, ROOT_LAYER as EMBEDDING_ROOT_LAYER, SCREEN_SIZE,
};

use super::*;

/// The layers the embedding root shows in its one window for a frame at `time`.
fn drawn(root: &mut Root, time: u64) -> Vec<Layer> {
    let done = root
        .frame(FrameRequest { time, wall_clock: 0 }, Instant::now() + PATIENT)
        .expect("the root draws");
    let list = LayerList::decode(&done.layers).expect("the frame is a layer list");
    let [window] = &list.windows[..] else {
        panic!("the root shows one window, not {:?}", list.windows);
    };
    assert_eq!(window.layers[0].id, EMBEDDING_ROOT_LAYER);
    assert_eq!(window.layers[0].surface, Surface::Root(0));
    window.layers.clone()
}

/// The editor's screen in a frame that shows the root's surface and the screen over it, with no
/// late marker: the frame waited for the app's answer, not for its deadline.
fn shown_screen(root: &mut Root, time: u64) -> Layer {
    let layers = drawn(root, time);
    let [_, screen] = &layers[..] else {
        panic!(
            "the frame answered before its deadline and shows the editor's screen over the \
             root's surface, not {layers:?}"
        );
    };
    screen.clone()
}

#[test]
fn a_root_built_on_the_library_shows_the_editor_it_embeds() {
    let mut root = embedding();
    let screen = shown_screen(&mut root, 1);
    let Surface::Screen { instance, surface } = screen.surface else {
        panic!(
            "the editor's layer shows a screen, not {:?}",
            screen.surface
        );
    };
    let pixels = (SCREEN_SIZE.0 * RASTER, SCREEN_SIZE.1 * RASTER);
    assert_eq!(screen.size, pixels, "the screen is painted at its raster");
    assert_eq!(screen.transform.origin(), pos2(ORIGIN.0, ORIGIN.1));
    let points = 1.0 / RASTER as f32;
    assert_eq!(
        (screen.transform.xx, screen.transform.yy),
        (points, points),
        "the layer scales the screen's pixels to its points"
    );
    let presented = root.surface(instance, surface).expect(
        "the host holds what the triangle presented to its screen, by the handle the root named",
    );
    assert_eq!(
        (presented.texture.width(), presented.texture.height()),
        pixels,
        "the triangle drew its screen at the size the root asked for, on the host's device"
    );

    assert_eq!(
        shown_screen(&mut root, 2),
        screen,
        "with no frame due, the next frame answers at once and shows the screen as it was"
    );

    assert_eq!(
        drawn(&mut root, CLOSES).len(),
        1,
        "the root let go of the app"
    );
    // A frame more, so the host has served what the root did as it let go.
    drawn(&mut root, CLOSES);
    assert!(
        root.surface(instance, surface).is_none(),
        "the app stopped once the root let go of it, though it sent nothing more"
    );
}
