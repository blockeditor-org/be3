use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn closing_a_dock_tab_asks_its_owner_to_remove_it() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let closed = Rc::new(RefCell::new(Vec::new()));
    let asked = closed.clone();
    let document = build(move || {
        let tabs = (1..=3).map(TabId::new).collect::<Vec<_>>();
        let (state, set_state) = create_signal(unstyled::DockState::new(tabs));
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: unstyled::DockState| set_state.set(next)}
                on_close={move |tab: TabId| asked.borrow_mut().push(tab)}
            >
                {move |tab: TabId| view! {
                    <Frame @test_id={format!("content.{}", tab.value())} />
                }}
            </styled::DockArea>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.frame(vec![Event::PointerMoved(tab)]);
    harness.middle_button(tab, true);
    harness.middle_button(tab, false);
    harness.frame(Vec::new());

    assert_eq!(*closed.borrow(), vec![TabId::new(2)], "its owner is asked");
    assert_eq!(
        dock_state(harness.document(), dock).all_tabs(),
        vec![TabId::new(1), TabId::new(2), TabId::new(3)],
        "the tab stays until its owner removes it"
    );
}
