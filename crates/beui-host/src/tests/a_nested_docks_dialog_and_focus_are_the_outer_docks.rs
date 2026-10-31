use super::*;
use crate::dock::unstyled::{DockDialog, DockPane, DockSplit, DockTab, DockingLayout};

#[test]
fn a_nested_docks_dialog_and_focus_are_the_outer_docks() {
    let _root = crate::reactive::RootScope::new();
    let built = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&built);
    let document = build_node(move || {
        let outer = DockingLayout::<u64>::new();
        let inner = DockingLayout::<u64>::new();
        let closed = create_signal(false);
        handle.set(Some((outer, closed)));
        let dialogs = create_memo(move || match closed.get() {
            true => Vec::new(),
            false => vec![12u64],
        });
        view! {
            <dock::styled::Docking layout=outer focus=2u64>
                <DockSplit id="root" fraction=0.3>
                    <DockPane id="side">
                        <DockTab id=1u64 title="Side">
                            <Frame @test_id="content.1" />
                        </DockTab>
                    </DockPane>
                    <DockPane id="main">
                        <DockTab id=2u64 title="Nested">
                            <dock::styled::Docking layout=inner focus=11u64>
                                <DockPane id="inner">
                                    <DockTab id=10u64 title="Inner 10">
                                        <Frame @test_id="content.10" />
                                    </DockTab>
                                    <DockTab id=11u64 title="Inner 11">
                                        <Frame @test_id="content.11" />
                                    </DockTab>
                                </DockPane>
                                <ForEach keys={dialogs}>|id: u64|
                                    <DockDialog
                                        id
                                        parent=11u64
                                        title="Inner dialog"
                                        size={vec2(200.0, 120.0)}
                                        on_close={move || closed.set(true)}
                                    >
                                        <Frame @test_id="content.12" />
                                    </DockDialog>
                                </ForEach>
                            </dock::styled::Docking>
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let (outer, closed) = built.get().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    for _ in 0..3 {
        harness.frame(Vec::new());
    }

    let state = outer.state().get_untracked();
    let group = state
        .find_group("joined:2")
        .expect("the inner dock is a group of the outer dock");
    let focused = state.focused_tab().expect("a tab is focused");
    assert!(
        state.group_tabs(group).contains(&focused),
        "the outer dock focuses the inner dock's focused tab"
    );
    assert!(
        harness.document().find_test_id("content.11").is_some()
            && harness.document().find_test_id("content.10").is_none(),
        "the inner dock's focus picks the tab its group shows"
    );

    let dialog = harness.rect(harness.find("content.12"));
    let parent = harness.rect(harness.find("content.11"));
    assert!(
        parent.contains_rect(dialog) && dialog.top() < parent.center().y,
        "the outer dock presents the inner dock's dialog hanging from its parent's tab, over its parent: {dialog:?} over {parent:?}"
    );

    let close = (1..32)
        .find_map(|tab| {
            harness
                .document()
                .find_test_id(&format!("dock.dialog.{tab}.close"))
        })
        .expect("the outer dock draws the dialog's close button");
    harness.click(harness.center(close));
    for _ in 0..3 {
        harness.frame(Vec::new());
    }
    assert!(
        closed.get_untracked(),
        "closing the dialog in the outer dock reaches the inner dock's on_close"
    );
    assert!(harness.document().find_test_id("content.12").is_none());
}
