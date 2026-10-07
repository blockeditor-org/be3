use super::*;
use crate::reactive::with_reactive_scope;
use crate::unstyled::{DockMode, DockPane, DockSplit, DockTab, DockingLayout};

#[test]
fn a_stacked_dock_fills_itself_with_the_focused_tab_and_keeps_its_panels() {
    let (mode, set_mode) = create_signal(DockMode::Stacked);
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking layout mode={mode} focus=3u64>
                <DockSplit id="split">
                    <DockPane id="left">
                        <DockTab id=1u64 title="Tab 1">
                            <Frame @test_id="content.1" />
                        </DockTab>
                        <DockTab id=2u64 title="Tab 2">
                            <Frame @test_id="content.2" />
                        </DockTab>
                    </DockPane>
                    <DockPane id="right">
                        <DockTab id=3u64 title="Tab 3" on_close={|| {}}>
                            <Frame @test_id="content.3" />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let third = harness
        .document()
        .find_test_id("content.3")
        .expect("the stacked dock shows the focused tab");
    let bar = harness.rect(harness.find("dock.switch"));
    assert_eq!(
        harness.rect(third),
        Rect::from_min_max(
            pos2(0.0, harness.rect(third).top()),
            pos2(WIDE_VIEWPORT.x, WIDE_VIEWPORT.y)
        ),
        "the focused tab fills the dock below its bar"
    );
    assert!(harness.rect(third).top() >= bar.bottom());
    assert!(
        harness.document().find_test_id("content.1").is_none(),
        "the pane beside it is not drawn"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.3.close")
            .is_none(),
        "a stacked dock draws no tab bars"
    );

    let tiling = set_mode.clone();
    with_reactive_scope(harness.document_mut(), move || tiling.set(DockMode::Tiled));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().find_test_id("content.3"),
        Some(third),
        "tiling the dock again keeps the panel it was showing"
    );
    assert!(
        harness.rect(third).left() > WIDE_VIEWPORT.x / 3.0,
        "the tab is back in the pane it was split into"
    );
    harness.find("content.1");

    with_reactive_scope(harness.document_mut(), move || {
        set_mode.set(DockMode::Stacked)
    });
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().find_test_id("content.3"),
        Some(third),
        "stacking it again shows the same panel"
    );
    assert_eq!(
        harness.rect(third).width(),
        WIDE_VIEWPORT.x,
        "stacking it again fills the dock with the same panel"
    );
}
