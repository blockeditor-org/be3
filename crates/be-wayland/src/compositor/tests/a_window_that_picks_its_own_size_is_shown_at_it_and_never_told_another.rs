use super::*;

#[component]
fn Floating(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    view! {
        <List spacing=Space::None>
            <ForEach keys={ids}>|id: WindowId| {
                let list = windows.list();
                let size = create_memo(move || {
                    list.with(|list| {
                        list.iter()
                            .find(|info| info.id == id)
                            .map_or(Vec2::ZERO, |info| info.size)
                    })
                });
                let width = create_memo(move || Size::Custom(size.get().x));
                let height = create_memo(move || Size::Custom(size.get().y));
                view! {
                    <Frame width=width height=height>
                        <WindowView windows id />
                    </Frame>
                }
            }</ForEach>
        </List>
    }
}

#[test]
fn a_window_that_picks_its_own_size_is_shown_at_it_and_never_told_another() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Floating windows />
        }
    });
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

    harness.client.attach_unsent(&window, 1, 1);
    harness.settle();
    harness.acknowledge(&window);
    assert_eq!(harness.window(id).size(), Vec2::new(1.0, 1.0));

    harness.client.attach_unsent(&window, 300, 200);
    harness.settle();
    harness.acknowledge(&window);
    assert_eq!(
        harness.window(id).size(),
        Vec2::new(300.0, 200.0),
        "the window is shown at the size it settled on"
    );
    assert_eq!(
        harness.client.received.size,
        Some((0, 0)),
        "being shown at its own size never pins the window to a size"
    );
}
