use super::*;
use crate::dock::{DockPane, DockSplit, DockTab, DockingLayout, SplitSize};
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
        let layout = DockingLayout::<u64>::new();
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
                                let inner = DockingLayout::<u64>::new();
                                view! {
                                    <dock::Docking layout=inner focus=10u64>
                                        <DockSplit id="editor" size={SplitSize::Fraction(0.7)}>
                                            <DockPane id="stage" locked=true bare=true>
                                                <DockTab id=10u64 title="Canvas">
                                                    <Frame @test_id="content.stage" />
                                                </DockTab>
                                            </DockPane>
                                            <DockPane id="side" locked=true>
                                                <DockTab id=11u64 title="Inspector">
                                                    <Frame @test_id="content.side" />
                                                </DockTab>
                                            </DockPane>
                                        </DockSplit>
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
        find_text(harness.document(), root, title, Match::Exact)
            .is_some_and(|node| harness.document().node_rect(node).is_some())
    };
    assert!(
        named("Report") && !named("Canvas"),
        "the bar names the document, not its editor's stage tab"
    );
    assert_eq!(
        count(&harness, "dock.switch"),
        "1",
        "the document counts as the one open tab"
    );

    let at = harness.center(harness.find("dock.switch"));
    harness.click(at);
    harness.settle();
    let card = harness
        .accessible()
        .into_iter()
        .find(|node| node.author_id() == Some("dock.switcher.tab.2"))
        .map(|node| node.label().map(str::to_owned));
    assert_eq!(
        card,
        Some(Some("Report".to_owned())),
        "the switcher's card is the document, as the badge counts it"
    );
}
