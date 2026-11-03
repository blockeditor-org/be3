use super::*;

#[test]
fn a_window_that_picks_its_own_size_reports_it_and_is_told_the_size_it_is_placed_at() {
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
    assert_eq!(harness.client.received.size, Some((0, 0)));

    harness.client.attach_unsent(&window, 300, 200);
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
        Vec2::new(300.0, 200.0),
        "the size the window picked is its own"
    );
    assert_eq!(
        harness.client.received.size,
        Some((SHOWN.x as i32, SHOWN.y as i32)),
        "a placed window is told the size of where it is placed, even one that picked its own"
    );
    assert_eq!(harness.window(id).size(), SHOWN);

    harness.acknowledge(&window);
    harness
        .client
        .attach_unsent(&window, SHOWN.x as i32, SHOWN.y as i32);
    harness.settle();
    let answered = listed(&mut harness);
    assert_eq!(
        answered.size,
        Vec2::new(300.0, 200.0),
        "answering keeps the size it picked as its own"
    );
    assert_eq!(answered.drawn, SHOWN, "and it is drawn at the size it answered");
}
