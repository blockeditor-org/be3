use be_sdk_root_testing::ROOT_LAYER as EMBEDDING_ROOT_LAYER;

use super::*;

/// The layers the embedding root shows in its one window for a frame at `time`.
fn drawn(root: &mut Root, time: u64) -> Vec<be_lib_layers::Layer> {
    let done = root
        .frame(
            FrameRequest {
                time,
                wall_clock: 0,
                utc_offset: 0,
                window: window(),
                describe: false,
            },
            Instant::now() + PATIENT,
        )
        .expect("the root draws");
    let list = LayerList::decode(&done.layers).expect("the frame is a layer list");
    let [window] = &list.windows[..] else {
        panic!("the root shows one window, not {:?}", list.windows);
    };
    assert_eq!(window.layers[0].id, EMBEDDING_ROOT_LAYER);
    window.layers.clone()
}

/// An app the host stops for breaking what it asks of it (be-host's `serve_editors`, for an
/// editor whose frames broke the app protocol) stops toward the root as well: the host no
/// longer runs it or holds what it presented, and the root, hearing it stopped, draws on
/// without it.
#[test]
fn an_app_its_host_stops_stops_toward_the_root_which_draws_on() {
    let mut root = embedding();
    let layers = drawn(&mut root, 1);
    let Some(Surface::Screen { instance, surface }) = layers.last().map(|layer| layer.surface)
    else {
        panic!("the root shows the triangle's screen over its own, not {layers:?}");
    };
    assert_eq!(
        root.app_of(instance).as_deref(),
        Some(be_sdk_root_testing::APP)
    );

    root.stop_instance(instance, "it broke the app protocol");

    assert_eq!(root.app_of(instance), None, "the host no longer runs it");
    assert!(
        root.surface(instance, surface).is_none(),
        "the host let go of what it presented"
    );
    for time in 2..5 {
        drawn(&mut root, time);
    }
}
