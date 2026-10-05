use super::*;
use crate::reactive::{Show, clone};

#[test]
fn a_dock_with_no_tabs_shows_the_view_it_was_given_for_nothing_open() {
    let document = build(move || {
        let layout = unstyled::DockingLayout::new();
        let (open, set_open) = create_signal(true);
        view! {
            <styled::Docking layout>
                <unstyled::DockPane
                    id="tabs"
                    empty={move || view! {
                        <Frame @test_id={"nothing_open"} />
                    }}
                >
                    <Show condition={open}>
                        <unstyled::DockTab
                            id=1u64
                            title="Tab 1"
                            on_close={clone!(set_open -> move || set_open.set(false))}
                        >
                            <Frame @test_id={"content.1"} />
                        </unstyled::DockTab>
                    </Show>
                </unstyled::DockPane>
            </styled::Docking>
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
