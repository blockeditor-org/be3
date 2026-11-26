use super::*;
use be_sdk_api::Watched;

#[test]
fn a_window_bound_to_a_block_leaves_the_published_windows_without_a_frame() {
    let _root = RootScope::new();
    publish_windows();
    flush();
    let watched = |open| {
        be_host_base::watches::published()
            .iter()
            .any(|(published, _)| *published == open)
    };
    assert!(
        watched(be_sdk_api::WindowsWatch::OPEN) && watched(be_sdk_api::OpenProgramsWatch::OPEN),
        "the windows and their programs are published as soon as the effect runs, even empty"
    );

    let bound = crate::programs::bound_windows();
    let runs = std::rc::Rc::new(std::cell::Cell::new(0));
    create_effect({
        let runs = runs.clone();
        move || {
            bound.track();
            runs.set(runs.get() + 1);
        }
    });
    flush();
    let block = uuid::Uuid::from_u128(1);
    let window = HostWindowId(7);
    crate::programs::show(block, uuid::Uuid::from_u128(2), uuid::Uuid::from_u128(3), block);
    crate::programs::bind(block, window);
    flush();
    assert_eq!(
        bound.get_untracked(),
        vec![window],
        "a window bound to a block's editor is left out of what is published at once"
    );
    assert_eq!(
        runs.get(),
        2,
        "and what publishes the windows runs again, with no frame comparing a counter"
    );
}
