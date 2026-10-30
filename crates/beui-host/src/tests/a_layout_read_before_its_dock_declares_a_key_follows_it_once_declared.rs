use super::*;
use crate::dock::unstyled::{
    DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockTab, DockingLayout,
};
use crate::reactive::Memo;

const BOTTOM: DockBarPlacement = DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center);

#[test]
fn a_layout_read_before_its_dock_declares_a_key_follows_it_once_declared() {
    let _root = crate::reactive::RootScope::new();
    let read = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&read);
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        let tab: Memo<bool> = create_memo(move || layout.tab_of(&1).is_some());
        let placement = create_memo(move || layout.bar_placement(&9));
        let hidden = create_memo(move || layout.is_bar_hidden(&9));
        assert!(!tab.get_untracked() && placement.get_untracked().is_none());
        assert!(!hidden.get_untracked());
        handle.set(Some((layout, tab, placement, hidden)));
        view! {
            <dock::styled::Docking layout focus=1u64>
                <DockPane id="stage">
                    <DockTab id=1u64 title="One">
                        <Frame @test_id="content.1" />
                    </DockTab>
                </DockPane>
                <DockBar id=9u64 placement=BOTTOM>
                    <TestToolBar />
                </DockBar>
            </dock::styled::Docking>
        }
    });
    let (layout, tab, placement, hidden) = read.get().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        tab.get_untracked(),
        "a memo that asked for a key's tab before the dock declared it sees the tab once it is"
    );
    assert_eq!(
        placement.get_untracked(),
        Some(BOTTOM),
        "and one that asked for a bar's placement sees where the bar was declared"
    );
    layout.hide_bar(&9);
    harness.frame(Vec::new());
    assert!(
        hidden.get_untracked(),
        "and one that asked whether the bar was hidden follows it being hidden"
    );
}
