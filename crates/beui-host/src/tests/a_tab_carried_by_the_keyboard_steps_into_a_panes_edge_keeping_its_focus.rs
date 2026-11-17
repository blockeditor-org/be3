use super::*;
use crate::dock::{DockPane, DockSplit, DockTab, DockingLayout};
use crate::dock_host::unstyled::DockApi;
use crate::reactive::*;

fn said(harness: &Harness) -> Vec<String> {
    harness
        .accessible()
        .into_iter()
        .filter(|node| node.role() == accesskit::Role::Status)
        .filter_map(|node| node.label().map(str::to_owned))
        .collect()
}

/// The focusable node within `node`'s nearest ancestor that holds one: a dock tab's button.
fn focusable_around(document: &Document, node: NodeId) -> NodeId {
    let mut at = node;
    loop {
        if let Some(first) = document.focusables_within(at).first() {
            return *first;
        }
        at = document.node_parent(at).expect("a focusable is around the tab");
    }
}

/// A dock tab with the keyboard's focus lifts with Move, steps between the dock's places with
/// the arrows, saying each ("right of Tab 3"), and drops into the one chosen with Enter. Carried
/// off its bar it keeps the focus: its row is hidden and its slot closes, but it is not
/// collapsed.
#[test]
fn a_tab_carried_by_the_keyboard_steps_into_a_panes_edge_keeping_its_focus() {
    let _root = RootScope::new();
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        let tab = move |id: u64| {
            view! {
                <DockTab id title={format!("Tab {id}")}>
                    <Interactive focusable=true>
                        <Frame @test_id={format!("content.{id}")} />
                    </Interactive>
                </DockTab>
            }
        };
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <dock::Docking layout>
                    <DockSplit id="split">
                        <DockPane id="left">{tab(1)}{tab(2)}</DockPane>
                        <DockPane id="right">{tab(3)}</DockPane>
                    </DockSplit>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let button = focusable_around(
        harness.document(),
        dock_tab(harness.document(), dock, "Tab 2"),
    );
    harness.document_mut().focus_focusable(button);
    harness.settle();

    assert!(
        run_dock_action(&mut harness, "drag.move"),
        "a focused tab offers Move"
    );
    harness.settle();
    let mut steps = 0;
    while !said(&harness)
        .iter()
        .any(|said| said.starts_with("Over right of Tab 3"))
    {
        steps += 1;
        assert!(
            steps <= 12,
            "the arrows reach the right edge of the other pane: {:?}",
            said(&harness)
        );
        harness.key(Key::ArrowRight, Modifiers::NONE);
        harness.settle();
    }
    assert_eq!(
        harness.document().focused_node(),
        Some(button),
        "carried off its bar, the tab keeps the focus"
    );
    let placed = harness.document().node_rect(button);
    assert!(
        placed.is_some_and(|rect| rect.width() <= 0.5),
        "placed, not collapsed, but taking no room in its bar: {placed:?}"
    );
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frames_to_settle();
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.leaves(state.main()).len(),
        3,
        "Enter drops it there, splitting the other pane"
    );
    let at = |id: u64| state.find(crate::dock::TabId::new(id)).map(|at| at.leaf);
    assert_ne!(at(2), at(3), "in a pane of its own");
    let content = harness.find("content.2");
    assert_eq!(
        harness.document().focused_node(),
        harness.document().node_parent(content),
        "dropped, its row built again in its new bar, the keyboard goes to the tab's content"
    );
    let right = harness.rect(harness.find("content.2"));
    let gamma = harness.rect(harness.find("content.3"));
    assert!(
        right.left() >= gamma.right() - 0.5,
        "on the right of tab 3: {right:?} {gamma:?}"
    );
}
