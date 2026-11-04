use super::*;

#[test]
fn a_placeholder_commit_is_not_a_size_the_window_picked() {
    let mut harness = Harness::new();
    let window = harness.client.toplevel();
    harness.settle();
    let id = *harness
        .compositor()
        .server()
        .state
        .windows()
        .last()
        .expect("the toplevel opened a window");
    harness.acknowledge(&window);

    harness.client.attach_unsent(&window, 2, 2);
    harness.settle();
    let listed = |harness: &mut Harness| {
        harness
            .compositor()
            .windows()
            .list()
            .get_untracked()
            .into_iter()
            .find(|info| info.id == id)
            .expect("the window is listed")
    };
    assert_eq!(
        listed(&mut harness).size,
        Vec2::ZERO,
        "a 2×2 commit (Firefox's 1×1 buffer on a display at scale 2) is listed as no size picked"
    );

    harness.acknowledge(&window);
    harness
        .client
        .attach_unsent(&window, SHOWN.x as i32, SHOWN.y as i32);
    harness.settle();
    harness.client.attach_unsent(&window, 300, 200);
    harness.settle();
    assert_eq!(
        listed(&mut harness).size,
        Vec2::new(300.0, 200.0),
        "a size it picks later is its own"
    );
}
