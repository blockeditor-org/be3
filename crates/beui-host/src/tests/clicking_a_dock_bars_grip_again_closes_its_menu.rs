use super::*;
use crate::dock::unstyled::{
    DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockTab, DockingLayout,
};

#[test]
fn clicking_a_dock_bars_grip_again_closes_its_menu() {
    for placement in [
        DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
        DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center),
        DockBarPlacement::Window {
            edge: DockEdge::Top,
            origin: pos2(200.0, 150.0),
        },
    ] {
        let _root = crate::reactive::RootScope::new();
        let document = build_node(move || {
            let layout = DockingLayout::<u64>::new();
            view! {
                <dock::styled::Docking layout focus=1u64>
                    <DockPane id="stage">
                        <DockTab id=1u64 title="One">
                            <Frame @test_id="content.1" />
                        </DockTab>
                    </DockPane>
                    <DockBar id=9u64 placement>
                        <TestToolBar />
                    </DockBar>
                </dock::styled::Docking>
            }
        });
        let mut harness = Harness::sized(document, WIDE_VIEWPORT);
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let menu_open = |harness: &Harness| {
            let root = harness.document().root().expect("the document has a root");
            text_within(harness.document(), root, "Pop out into a window").is_some()
        };

        harness.click(harness.center(harness.find("dock.bar.9.grip")));
        harness.frame(Vec::new());
        assert!(
            menu_open(&harness),
            "the first click opens the grip's menu ({placement:?})"
        );

        harness.click(harness.center(harness.find("dock.bar.9.grip")));
        for _ in 0..4 {
            harness.frame(Vec::new());
        }
        assert!(
            !menu_open(&harness),
            "a second click on the grip closes its menu rather than closing and opening it again ({placement:?})"
        );
    }
}
