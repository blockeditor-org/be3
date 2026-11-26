use super::*;
use crate::dock::{DockPane, DockTab, DockingLayout};
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::PopoverHandle;

#[test]
fn a_press_on_a_tab_that_closes_a_popover_in_another_shows_the_tab() {
    let _root = crate::reactive::RootScope::new();
    let popover = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout>
                    <DockPane id="tabs">
                        <DockTab id=1u64 title="One">
                            <Popover @node_ref=&popover label="Tools">|_: PopoverHandle|
                                <Text @test_id="tools" color=Color32::WHITE>"Tools"</Text>
                            </Popover>
                        </DockTab>
                        <DockTab id=2u64 title="Two">
                            <Text @test_id="two" color=Color32::WHITE>"Two"</Text>
                        </DockTab>
                    </DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let trigger = harness
        .document()
        .api::<unstyled::PopoverApi>(popover.get())
        .trigger();
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("tools").is_some(),
        "the popover opens"
    );

    harness.click(harness.center(harness.find("dock.tab.2")));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("tools").is_none(),
        "the press closes it"
    );
    assert!(
        harness.document().find_test_id("two").is_some(),
        "and goes on to the tab it landed on"
    );
}
