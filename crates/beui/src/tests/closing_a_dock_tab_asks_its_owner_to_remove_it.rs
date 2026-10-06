use super::*;
use crate::reactive::{ForEach, clone};
use crate::unstyled::{DockPane, DockTab, DockingLayout, TabId, dock_state};

#[test]
fn closing_a_dock_tab_asks_its_owner_to_remove_it() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let closed = Rc::new(RefCell::new(Vec::new()));
    let asked = closed.clone();
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking @node_ref=&built layout>
                <DockPane id="tabs">
                    <ForEach keys={vec![1u64, 2, 3]}>
                        {move |id: u64| clone!(asked -> view! {
                            <DockTab
                                id
                                title={format!("Tab {id}")}
                                on_close={move || asked.borrow_mut().push(TabId::new(id))}
                            >
                                <Frame @test_id={format!("content.{id}")} />
                            </DockTab>
                        })}
                    </ForEach>
                </DockPane>
            </styled::Docking>
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
