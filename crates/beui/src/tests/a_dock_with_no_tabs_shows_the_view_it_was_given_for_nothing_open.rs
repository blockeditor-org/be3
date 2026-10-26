use super::*;
use crate::reactive::*;

#[test]
fn a_dock_with_no_tabs_shows_the_view_it_was_given_for_nothing_open() {
    let document = build_node(move || {
        let layout = dock::unstyled::DockingLayout::new();
        let open = create_signal(true);
        view! {
            <dock::styled::Docking layout>
                <dock::unstyled::DockPane
                    id="tabs"
                    empty={move || view! {
                        <Frame @test_id={"nothing_open"} />
                    }}
                >
                    <Show condition={open}>
                        <dock::unstyled::DockTab
                            id=1u64
                            title="Tab 1"
                            on_close={move || open.set(false)}
                        >
                            <Frame @test_id={"content.1"} />
                        </dock::unstyled::DockTab>
                    </Show>
                </dock::unstyled::DockPane>
            </dock::styled::Docking>
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
