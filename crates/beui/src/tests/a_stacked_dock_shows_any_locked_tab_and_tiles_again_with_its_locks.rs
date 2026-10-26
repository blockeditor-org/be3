use super::*;
use crate::dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout, TabId};

#[test]
fn a_stacked_dock_shows_any_locked_tab_and_tiles_again_with_its_locks() {
    let _root = crate::reactive::RootScope::new();
    let layout = DockingLayout::<u64>::new();
    let document = build_node(move || {
        view! {
            <dock::styled::Docking layout focus=10u64>
                <DockSplit id="workspace" fraction=0.3>
                    <DockPane id="files" locked=true>
                        <DockTab id=1u64 title="Tab 1"><Frame /></DockTab>
                    </DockPane>
                    <DockPane id="editors" locked=true>
                        <DockTab id=10u64 title="Tab 10"><Frame /></DockTab>
                        <DockTab id=11u64 title="Tab 11"><Frame /></DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);
    harness.frame(Vec::new());
    assert_eq!(layout.mode().get_untracked(), dock::unstyled::DockMode::Stacked);

    layout.show(&1);
    harness.frame(Vec::new());
    assert_eq!(layout.shown(), Some(1), "the stacked dock shows a sidebar tab");
    layout.show(&11);
    harness.frame(Vec::new());
    assert_eq!(layout.shown(), Some(11), "and a content tab, one at a time");

    harness.resize(WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert_eq!(layout.mode().get_untracked(), dock::unstyled::DockMode::Tiled);
    let state = layout.state().get_untracked();
    for tab in [1, 10, 11] {
        assert!(
            state.is_tab_pinned(TabId::new(tab)) && state.at_home(TabId::new(tab)),
            "tab {tab} tiles again locked in its area"
        );
    }
    let files = state.find(TabId::new(1)).expect("tab 1 is open").leaf;
    assert_eq!(state.area_of(files), Some("files"));
}
