use super::*;
use crate::dock::{DockPane, DockSplit, DockTab, DockingLayout};
use crate::dock_host::unstyled::DockApi;
use crate::reactive::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// A tab whose panel holds an app's view (an `Embed`) carried into another pane keeps its
/// panel: nothing in it is built again, and the app is placed at one new size, once, when the
/// drop commits; while carried it keeps the size it was laid out at.
#[test]
fn an_app_panel_moved_by_a_drop_keeps_its_state_and_resizes_once() {
    let _root = RootScope::new();
    let slot = EmbedSlot::new();
    let builds = create_signal(0_u32);
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <dock::Docking layout>
                    <DockSplit id="split" size={crate::dock::SplitSize::Fraction(0.3)}>
                        <DockPane id="left">
                            <DockTab id=1u64 title="Tab 1">
                                <Frame @test_id="content.1" />
                            </DockTab>
                        </DockPane>
                        <DockPane id="right">
                            <DockTab id=2u64 title="Tab 2">
                                <Frame @test_id="content.2" />
                            </DockTab>
                            <DockTab
                                id=3u64
                                title="Tab 3"
                                content={move || {
                                    builds.update(|builds| *builds += 1);
                                    view! {
                                        <Embed slot />
                                    }
                                }}
                            />
                        </DockPane>
                    </DockSplit>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 3")));
    harness.settle();
    let placed = slot
        .placement()
        .expect("the app is placed in its pane")
        .rect;
    let sizes = Rc::new(RefCell::new(Vec::<Option<Vec2>>::new()));
    {
        let sizes = Rc::clone(&sizes);
        slot.on_place(move |placement| {
            let size = placement.map(|placement| placement.rect.size());
            let mut sizes = sizes.borrow_mut();
            if sizes.last() != Some(&size) {
                sizes.push(size);
            }
        });
    }

    let from = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let onto = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    harness.down(from, PointerButton::Primary);
    for step in 1..=8 {
        harness.hover(from + (onto - from) * (step as f32 / 8.0));
    }
    harness.pause(Duration::from_millis(400));
    assert!(
        sizes
            .borrow()
            .iter()
            .all(|size| *size == Some(placed.size())),
        "while carried the app keeps the size it was laid out at: {:?}",
        sizes.borrow()
    );
    harness.up(onto, PointerButton::Primary);
    harness.frames_to_settle();

    let state = harness.document().api::<DockApi>(dock).state();
    let leaf_of = |id: u64| state.find(crate::dock::TabId::new(id)).map(|at| at.leaf);
    assert_eq!(
        leaf_of(3),
        leaf_of(1),
        "let go on the other bar, the tab moves into that pane"
    );
    assert_eq!(builds.get_untracked(), 1, "its panel is not built again");
    let after = slot
        .placement()
        .expect("the app is placed in its new pane")
        .rect;
    assert_ne!(after.size(), placed.size(), "it takes the new pane's size");
    assert_eq!(
        *sizes.borrow(),
        [Some(after.size())],
        "placed once at its new size, never unplaced on the way"
    );
}
