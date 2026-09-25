use super::*;

#[test]
fn a_docked_pane_lays_its_content_inside_its_border() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let (state, set_state) = create_signal(unstyled::DockState::new([unstyled::TabId::new(1)]));
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                title={Func::new(|tab: unstyled::TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: unstyled::DockState| set_state.set(next)}
                on_close={move |_: unstyled::TabId| {}}
            >
                {move |_: unstyled::TabId| view! {
                    <Frame @test_id="content" />
                }}
            </styled::DockArea>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let pane = harness.rect(dock);
    let content = harness.rect(harness.find("content"));
    let border = styled::CHROME_BORDER;
    assert_eq!(content.min.x, pane.min.x + border);
    assert_eq!(content.max.x, pane.max.x - border);
    assert_eq!(content.max.y, pane.max.y - border);
}
