use super::*;
use crate::dock::{DockPane, DockTab, DockingLayout};
use crate::reactive::*;

fn count(harness: &Harness, test_id: &str) -> String {
    let document = harness.document();
    let mut pending = vec![harness.find(test_id)];
    while let Some(node) = pending.pop() {
        if let Some(text) = document.arena.kind_of::<TextNode>(node) {
            return document.text(text).to_owned();
        }
        pending.extend(document.children(node));
    }
    String::new()
}

#[test]
fn a_stacked_dock_names_and_counts_a_joined_docks_stage_as_the_tab_it_joined() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout focus=2u64>
                    <DockPane id="tabs">
                        <DockTab id=1u64 title="Files">
                            <Frame @test_id="content.files" />
                        </DockTab>
                        <DockTab
                            id=2u64
                            title="Report"
                            on_close={|| {}}
                            content={move || {
                                let inner = DockingLayout::new();
                                view! {
                                    <dock::Docking layout=inner focus=10u64>
                                        <DockPane id="stage" locked=true bare=true>
                                            <DockTab id=10u64 title="Text">
                                                <Frame @test_id="content.stage" />
                                            </DockTab>
                                        </DockPane>
                                    </dock::Docking>
                                }
                            }}
                        />
                    </DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);
    harness.settle();
    harness.settle();
    let root = harness.document().root().expect("the document has a root");
    let named = |title: &str| {
        text_within(harness.document(), root, title)
            .is_some_and(|node| harness.document().node_rect(node).is_some())
    };
    assert!(
        named("Report") && !named("Text"),
        "the bar names the document, not its editor's stage tab"
    );
    assert_eq!(
        count(&harness, "dock.switch"),
        "1",
        "the document counts as the one open tab"
    );
}
