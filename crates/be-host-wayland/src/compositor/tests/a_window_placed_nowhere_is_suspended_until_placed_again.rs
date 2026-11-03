use super::*;

thread_local! {
    static PLACED: std::cell::Cell<Option<Signal<bool>>> = const { std::cell::Cell::new(None) };
}

#[component]
fn Placeable(windows: Windows) -> NodeId {
    let placed = create_signal(true);
    PLACED.with(|held| held.set(Some(placed)));
    let ids = create_memo(clone_list(&windows));
    view! {
        <Layers>
            <Show condition={placed}>
            <List spacing=Space::None>
                <ForEach keys={ids}>|id: WindowId|
                    <Frame width={Size::Custom(SHOWN.x)} height={Size::Custom(SHOWN.y)}>
                        <WindowView windows id />
                    </Frame>
                </ForEach>
            </List>
            </Show>
        </Layers>
    }
}

fn place(harness: &mut Harness, placed: bool) {
    PLACED
        .with(std::cell::Cell::get)
        .expect("the view was built")
        .set(placed);
    harness.settle();
}

#[test]
fn a_window_placed_nowhere_is_suspended_until_placed_again() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Placeable windows />
        }
    });
    let (_window, id) = harness.open();
    assert!(!harness.client.received.suspended);

    place(&mut harness, false);
    assert!(
        harness.client.received.suspended,
        "a window placed nowhere is suspended, so its program can stop drawing"
    );
    assert!(harness.compositor().windows().suspended(id));
    assert_eq!(
        harness.client.received.size,
        Some((SHOWN.x as i32, SHOWN.y as i32)),
        "it keeps the size it was last placed at"
    );

    place(&mut harness, true);
    assert!(
        !harness.client.received.suspended,
        "placed again, it is no longer suspended"
    );
}
