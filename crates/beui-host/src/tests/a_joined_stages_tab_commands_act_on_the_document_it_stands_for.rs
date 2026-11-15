use super::*;
use crate::dock::{DockPane, DockTab, Docking, DockingLayout, Entry, TabId};

/// With a joined editor's stage focused, Next tab walks the documents beside it and Close tab
/// closes its document, not the stage's one tab (`DockState::standing_leaf`).
#[test]
fn a_joined_stages_tab_commands_act_on_the_document_it_stands_for() {
    let _root = crate::reactive::RootScope::new();
    let built = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&built);
    let document = build_node(move || {
        let outer = DockingLayout::new();
        let open = create_signal(vec![2u64, 3]);
        handle.set(Some((outer, open)));
        view! {
            <dock_host::styled::DockHost>
                <Docking layout=outer focus=2u64>
                    <DockPane id="main">
                        <ForEach value={open}>|id: u64| {
                            let close = move || open.update(|open| open.retain(|other| *other != id));
                            match id {
                                2 => view! {
                                    <DockTab id title="Document" on_close={close}>
                                        <Docking layout={DockingLayout::new()}>
                                            <DockPane id="stage" locked=true bare=true>
                                                <DockTab id=10u64 title="Stage">
                                                    <Frame @test_id="content.10" />
                                                </DockTab>
                                            </DockPane>
                                        </Docking>
                                    </DockTab>
                                },
                                _ => view! {
                                    <DockTab id title="Other" on_close={close}>
                                        <Frame @test_id="content.3" />
                                    </DockTab>
                                },
                            }
                        }</ForEach>
                    </DockPane>
                </Docking>
            </dock_host::styled::DockHost>
        }
    });
    let (outer, open) = built.get().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("content.10")));
    harness.frame(Vec::new());
    let state = outer.state().get_untracked();
    let group = state
        .find_group("joined:2")
        .expect("the document's dock joins its tab");
    let [joined] = state.group_tabs(group)[..] else {
        panic!("the stage holds its one tab");
    };
    let stage = state.find(joined).expect("the stage is open").leaf;
    assert_eq!(
        state.focused_leaf(),
        Some(stage),
        "the press focuses the stage"
    );
    let main = state.standing_leaf(stage);
    assert_eq!(
        state.entries(main),
        vec![Entry::Group(group), Entry::Tab(TabId::new(3))]
    );

    assert!(
        run_dock_action(&mut harness, "dock.tab.next"),
        "Next tab is offered from the stage, as the pane around it holds two documents"
    );
    harness.frame(Vec::new());
    assert_eq!(
        outer.state().get_untracked().active_entry(main),
        Some(Entry::Tab(TabId::new(3))),
        "Next tab shows the next document"
    );
    assert!(run_dock_action(&mut harness, "dock.tab.previous"));
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("content.10")));
    harness.frame(Vec::new());

    assert!(run_dock_action(&mut harness, "dock.tab.close"));
    harness.frame(Vec::new());
    assert_eq!(
        open.get_untracked(),
        vec![3],
        "Close tab closes the document the stage stands for"
    );
}
