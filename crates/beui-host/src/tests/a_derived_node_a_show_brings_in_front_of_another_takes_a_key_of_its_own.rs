use super::*;
use crate::dock::{DockPane, DockSpecNode, DockSplit, DockTab, Docking};
use crate::dock_host::unstyled::DockApi;

fn pane_keys(node: &DockSpecNode, keys: &mut Vec<String>) {
    match node {
        DockSpecNode::Split { first, second, .. } => {
            pane_keys(first, keys);
            pane_keys(second, keys);
        }
        DockSpecNode::Pane(pane) => keys.push(pane.key.clone()),
    }
}

/// A pane and a tab whose ids are derived from their places and titles keep the key they took
/// first, so a sibling a `Show` brings in front of each derives another instead of taking theirs.
#[test]
fn a_derived_node_a_show_brings_in_front_of_another_takes_a_key_of_its_own() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(false);
    let dock = NodeRef::new();
    let document = build_node(move || {
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <Docking>
                    <DockSplit>
                        <Show value={shown}>
                            <DockPane>
                                <DockTab title="Front">
                                    <Frame @test_id="content.front" />
                                </DockTab>
                            </DockPane>
                        </Show>
                        <DockPane>
                            <Show value={shown}>
                                <DockTab title="Same">
                                    <Frame @test_id="content.same.new" />
                                </DockTab>
                            </Show>
                            <DockTab title="Same">
                                <Frame @test_id="content.same" />
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let before = harness.document().api::<DockApi>(dock.get()).state();
    assert_eq!(before.all_tabs().len(), 1);
    let kept = before.all_tabs()[0];

    shown.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let after = harness.document().api::<DockApi>(dock.get()).state();
    let mut keys = Vec::new();
    pane_keys(after.spec().main.as_ref().expect("the dock has a tree"), &mut keys);
    let mut distinct = keys.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), keys.len(), "every pane has a key of its own: {keys:?}");
    let tabs = after.all_tabs();
    assert_eq!(tabs.len(), 3, "the tab of the same title is declared beside the first");
    assert!(tabs.contains(&kept), "the tab declared first keeps its id");
}
