use super::*;

thread_local! {
    static ASKS: std::cell::Cell<Option<Signal<u64>>> = const { std::cell::Cell::new(None) };
}

#[component]
fn Asked(windows: Windows) -> NodeId {
    let asks = create_signal(0u64);
    ASKS.with(|held| held.set(Some(asks)));
    let ids = create_memo(clone_list(&windows));
    let focus = create_memo(move || asks.get());
    view! {
        <List spacing=Space::None>
            <TextInput @test_id={"test.input"} value="" />
            <ForEach keys={ids}>|id: WindowId|
                <Frame width={Size::Custom(SHOWN.x)} height={Size::Custom(SHOWN.y)}>
                    <WindowView windows id focus />
                </Frame>
            </ForEach>
        </List>
    }
}

#[test]
fn a_window_asked_to_take_the_focus_by_its_placer_gets_the_keyboard() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Asked windows />
        }
    });
    let _keyboard = harness.client.keyboard();
    let (_window, id) = harness.open();
    harness.click("test.input");
    assert_eq!(harness.compositor().windows().focused(), None);
    assert!(!harness.client.received.keyboard_entered);

    ASKS.with(std::cell::Cell::get)
        .expect("the view was built")
        .update(|asks| *asks += 1);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(id),
        "the window its placer asked to focus has the focus"
    );
    assert!(
        harness.client.received.keyboard_entered,
        "and its program has the keyboard"
    );
}
