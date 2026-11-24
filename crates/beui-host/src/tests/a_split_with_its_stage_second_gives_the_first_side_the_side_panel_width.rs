use super::*;
use crate::dock::{
    DockPane, DockSpecNode, DockSplit, DockTab, Docking, DockingLayout, SIDE_PANEL_WIDTH,
    SplitSize,
};

/// A split given no size, with the stage on its second side, gives its first side the side
/// panel's width.
#[test]
fn a_split_with_its_stage_second_gives_the_first_side_the_side_panel_width() {
    let _root = crate::reactive::RootScope::new();
    let layout = DockingLayout::<u64>::new();
    let document = build_node(move || {
        view! {
            <dock_host::styled::DockHost>
                <Docking layout>
                    <DockSplit>
                        <DockPane>
                            <DockTab title="Panel">
                                <Frame />
                            </DockTab>
                        </DockPane>
                        <DockPane stage=true>
                            <DockTab>
                                <Frame />
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let spec = layout.state().get_untracked().spec();
    match spec.main {
        Some(DockSpecNode::Split { size, .. }) => {
            assert_eq!(size, SplitSize::First(SIDE_PANEL_WIDTH));
        }
        other => panic!("the dock is one split, not {other:?}"),
    }
}
