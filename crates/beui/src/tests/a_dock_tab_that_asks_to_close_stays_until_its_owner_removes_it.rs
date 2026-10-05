use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn a_dock_tab_that_asks_to_close_stays_until_its_owner_removes_it() {
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
                asks_to_close={Func::new(|tab: TabId| tab == TabId::new(2))}
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
    let middle_click = |harness: &mut Harness, title: &str| {
        let tab = harness.center(dock_tab(harness.document(), dock, title));
        harness.frame(vec![Event::PointerMoved(tab)]);
        harness.middle_button(tab, true);
        harness.middle_button(tab, false);
        harness.frame(Vec::new());
    };

    middle_click(&mut harness, "Tab 2");

    assert_eq!(*closed.borrow(), vec![TabId::new(2)], "its owner is asked");
    assert_eq!(
        dock_state(harness.document(), dock).all_tabs(),
        vec![TabId::new(1), TabId::new(2), TabId::new(3)],
        "the tab stays until its owner removes it"
    );

    middle_click(&mut harness, "Tab 3");

    assert_eq!(
        dock_state(harness.document(), dock).all_tabs(),
        vec![TabId::new(1), TabId::new(2)],
        "any other tab closes at once"
    );
}
