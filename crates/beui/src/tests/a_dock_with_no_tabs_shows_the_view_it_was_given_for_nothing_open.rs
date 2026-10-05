use super::*;

#[test]
fn a_dock_with_no_tabs_shows_the_view_it_was_given_for_nothing_open() {
    let document = build(move || {
        let (state, set_state) = create_signal(unstyled::DockState::new([unstyled::TabId::new(1)]));
        let removing = set_state.clone();
        view! {
            <styled::DockArea
                state={state}
                title={Func::new(|tab: unstyled::TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: unstyled::DockState| set_state.set(next)}
                on_close={move |tab: unstyled::TabId| removing.update(|state| {
                    state.close(tab);
                })}
                empty={move || view! {
                    <Frame @test_id={"nothing_open"} />
                }}
            >
                {move |tab: unstyled::TabId| view! {
                    <Frame @test_id={format!("content.{}", tab.value())} />
                }}
            </styled::DockArea>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("nothing_open").is_none(),
        "a dock showing a tab does not show the view for nothing open"
    );

    let close = harness.center(harness.find("dock.tab.1.close"));
    harness.click(close);
    harness.frame(Vec::new());

    assert!(
        harness.document().find_test_id("content.1").is_none(),
        "the closed tab's content is gone"
    );
    harness.find("nothing_open");
}
