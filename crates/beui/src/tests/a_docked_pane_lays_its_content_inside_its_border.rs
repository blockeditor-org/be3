use super::*;

#[test]
fn a_docked_pane_lays_its_content_inside_its_border() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let layout = unstyled::DockingLayout::new();
        view! {
            <styled::Docking @node_ref=&built layout>
                <unstyled::DockPane id="tabs">
                    <unstyled::DockTab id=1u64 title="Tab 1">
                        <Frame @test_id="content" />
                    </unstyled::DockTab>
                </unstyled::DockPane>
            </styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let pane = harness.rect(dock);
    let content = harness.rect(harness.find("content"));
    let border = styled::DOCK_INSET + styled::CHROME_BORDER;
    assert_eq!(content.min.x, pane.min.x + border);
    assert_eq!(content.max.x, pane.max.x - border);
    assert_eq!(content.max.y, pane.max.y - border);
}
