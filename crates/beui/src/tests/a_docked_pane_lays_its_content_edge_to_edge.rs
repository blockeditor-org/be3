use super::*;

#[test]
fn a_docked_pane_lays_its_content_edge_to_edge() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = dock::unstyled::DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <dock::unstyled::DockPane id="tabs">
                    <dock::unstyled::DockTab id=1u64 title="Tab 1">
                        <Frame @test_id="content" />
                    </dock::unstyled::DockTab>
                </dock::unstyled::DockPane>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let pane = harness.rect(dock);
    let content = harness.rect(harness.find("content"));
    assert_eq!(
        content.min.x, pane.min.x,
        "no inset or border at the dock's edge"
    );
    assert_eq!(content.max.x, pane.max.x);
    assert_eq!(content.max.y, pane.max.y);
}
