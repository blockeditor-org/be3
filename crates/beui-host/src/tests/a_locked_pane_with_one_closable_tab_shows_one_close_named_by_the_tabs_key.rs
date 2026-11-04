use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

#[test]
fn a_locked_pane_with_one_closable_tab_shows_one_close_named_by_the_tabs_key() {
    let _root = crate::reactive::RootScope::new();
    let second = create_signal(false);
    let document = build_node(move || {
        let layout = dock::DockingLayout::<&'static str>::new();
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout>
                    <dock::DockSplit id="split">
                        <dock::DockPane id="side" locked=true>
                            <dock::DockTab id="notes" title="Notes" on_close={|| {}}>
                                <Frame @test_id={"content.notes"} />
                            </dock::DockTab>
                            <Show condition={second}>
                                <dock::DockTab id="outline" title="Outline" on_close={|| {}}>
                                    <Frame @test_id={"content.outline"} />
                                </dock::DockTab>
                            </Show>
                        </dock::DockPane>
                        <dock::DockPane id="stage" locked=true bare=true>
                            <dock::DockTab id="stage" title="Stage">
                                <Frame @test_id={"content.stage"} />
                            </dock::DockTab>
                        </dock::DockPane>
                    </dock::DockSplit>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.notes.close")
            .is_some(),
        "a tab's close button is named by the key it was declared with"
    );
    assert!(
        harness.document().find_test_id("dock.side.close").is_none(),
        "a pane whose only tab closes itself shows no second close button"
    );

    second.set(true);
    harness.settle();
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.outline.close")
            .is_some()
    );
    assert!(
        harness.document().find_test_id("dock.side.close").is_some(),
        "a pane of several closable tabs offers to close them all"
    );
}
