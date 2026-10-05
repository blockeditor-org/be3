use super::*;
use crate::reactive::{build, clone, create_memo, view};
use crate::unstyled::{
    Container, DockMode, DockPane, DockSplit, DockTab, DockingLayout, narrower_than,
};

const BREAKPOINT: f32 = 600.0;

#[test]
fn a_dock_stacked_by_narrowing_its_container_leaves_the_hidden_panel_unplaced() {
    let home = NodeRef::new();
    let document = build({
        let home = home.clone();
        move || {
            view! {
                <Container>
                    {move |_| {
                        let home = home.clone();
                        let narrow = narrower_than(BREAKPOINT);
                        let mode = create_memo(clone!(narrow -> move || match narrow.get() {
                            true => DockMode::Stacked,
                            false => DockMode::Tiled,
                        }));
                        let layout = DockingLayout::new();
                        view! {
                            <styled::Docking layout mode home=1u64 focus=2u64>
                                <DockSplit id="split">
                                    <DockPane id="left">
                                        <DockTab id=1u64 title="Tab 1">
                                            <Frame @node_ref=&home />
                                        </DockTab>
                                    </DockPane>
                                    <DockPane id="right">
                                        <DockTab id=2u64 title="Tab 2">
                                            <Frame />
                                        </DockTab>
                                    </DockPane>
                                </DockSplit>
                            </styled::Docking>
                        }
                    }}
                </Container>
            }
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let home = home.get();
    assert!(
        harness.document().node_rect(home).is_some(),
        "the tiled dock lays out the home tab beside the other"
    );

    *harness.viewport_mut() = VIEWPORT;
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().node_rect(home),
        None,
        "the stacked dock shows the other tab, so the home tab's panel has no place on screen"
    );
}
