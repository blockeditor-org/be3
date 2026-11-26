use super::*;
use crate::dock::{DockPane, DockTab, DockingLayout, TabId};
use crate::primitives::{KeepAlive, Kept};
use crate::reactive::*;
use crate::styled::{List, ListItem, Space};

#[test]
fn a_dock_kept_hidden_lets_its_tab_go_and_joins_again_when_shown() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let built = create_signal(0);
    let layouts = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&layouts);
    let document = build_node(move || {
        let outer = DockingLayout::new();
        handle.set(Some(outer));
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout=outer focus=2u64>
                    <DockPane id="main">
                        <DockTab id=2u64 title="Document">
                            <KeepAlive>
                                <List spacing=Space::None>
                                    <Show value={shown}>
                                        <ListItem grow=1.0>
                                            <Kept>|| {
                                                built.update(|built| *built += 1);
                                                let inner = DockingLayout::<u64>::new();
                                                view! {
                                                    <dock::Docking layout=inner>
                                                        <DockPane id="inner" bare=true>
                                                            <DockTab id=10u64 title="Stage">
                                                                <Frame @test_id="content.10" />
                                                            </DockTab>
                                                        </DockPane>
                                                    </dock::Docking>
                                                }
                                            }</Kept>
                                        </ListItem>
                                    </Show>
                                </List>
                            </KeepAlive>
                        </DockTab>
                    </DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let outer = layouts.take().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let joined = |outer: &DockingLayout<u64>| {
        outer
            .state()
            .with_untracked(|state| state.find_group("joined:2").is_some())
    };
    assert!(joined(&outer), "shown, the inner dock joins the tab");

    shown.set(false);
    harness.settle();
    assert!(!joined(&outer), "hidden, it lets the tab go");
    assert!(
        outer
            .state()
            .with_untracked(|state| state.contains(TabId::new(2))),
        "and the tab is back as itself"
    );

    shown.set(true);
    harness.settle();
    assert!(joined(&outer), "shown again, it joins again");
    assert_eq!(built.get_untracked(), 1, "kept, never built again");
}
