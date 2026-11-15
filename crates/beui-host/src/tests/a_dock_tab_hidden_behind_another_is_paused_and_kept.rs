use super::*;
use crate::dock::{DockPane, DockTab, DockingLayout};
use crate::reactive::*;

#[test]
fn a_dock_tab_hidden_behind_another_is_paused_and_kept() {
    let _root = crate::reactive::RootScope::new();
    let source = create_signal(0);
    let seen = create_signal([0, 0]);
    let builds = create_signal([0, 0]);
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout>
                    <DockPane id="tabs">
                        <ForEach value={vec![1u64, 2]}>|id: u64|
                            <DockTab
                                id
                                title={format!("Tab {id}")}
                                content={move || {
                                    let at = id as usize - 1;
                                    builds.update(|builds| builds[at] += 1);
                                    create_effect(move || {
                                        let now = source.get();
                                        seen.update(|seen| seen[at] = now);
                                    });
                                    view! {
                                        <Frame @test_id={format!("content.{id}")} width=40.0 height=30.0 />
                                    }
                                }}
                            />
                        </ForEach>
                    </DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert_eq!(builds.get_untracked(), [1, 0], "only the tab shown is built");

    harness.click(harness.center(harness.find("dock.tab.2")));
    harness.frame(Vec::new());
    assert_eq!(builds.get_untracked(), [1, 1]);
    assert!(harness.document().find_test_id("content.1").is_none(), "the first tab is hidden");

    source.set(1);
    harness.frame(Vec::new());
    assert_eq!(
        seen.get_untracked(),
        [0, 1],
        "the hidden tab's effects wait while the shown tab's run"
    );

    harness.click(harness.center(harness.find("dock.tab.1")));
    harness.frame(Vec::new());
    assert_eq!(builds.get_untracked(), [1, 1], "showing a tab again builds nothing");
    assert_eq!(
        seen.get_untracked(),
        [1, 1],
        "and what it missed while hidden runs once it shows"
    );
}
