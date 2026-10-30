use super::*;
use crate::dock::unstyled::{
    DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockSplit, DockTab, DockWindow,
    DockingLayout,
};
use crate::geometry::Rect;

#[test]
fn a_dock_bar_of_the_whole_dock_takes_its_band_from_every_pane_and_the_windows_area() {
    let _root = crate::reactive::RootScope::new();
    let built = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&built);
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        handle.set(Some(layout));
        view! {
            <dock::styled::Docking layout focus=1u64>
                <DockSplit id="root" fraction=0.6>
                    <DockPane id="stage">
                        <DockTab id=1u64 title="One">
                            <Frame @test_id="content.1" />
                        </DockTab>
                        <DockTab id=3u64 title="Three">
                            <Frame @test_id="content.3" />
                        </DockTab>
                    </DockPane>
                    <DockPane id="side">
                        <DockTab id=2u64 title="Side">
                            <Frame @test_id="content.2" />
                        </DockTab>
                    </DockPane>
                </DockSplit>
                <DockWindow
                    id="floating"
                    rect={Rect::from_min_size(pos2(500.0, 590.0), vec2(300.0, 200.0))}
                >
                    <DockPane id="floating">
                        <DockTab id=4u64 title="Floating">
                            <Frame @test_id="content.4" />
                        </DockTab>
                    </DockPane>
                </DockWindow>
                <DockBar id=9u64 placement={DockBarPlacement::Beside(DockEdge::Bottom)}>
                    <TestToolBar />
                </DockBar>
            </dock::styled::Docking>
        }
    });
    let layout = built.get().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let bar = harness.rect(harness.find("bar.9"));
    assert!(
        bar.left().abs() < 0.5
            && (bar.right() - WIDE_VIEWPORT.x).abs() < 0.5
            && (bar.bottom() - WIDE_VIEWPORT.y).abs() < 0.5,
        "docked beside the bottom, the dock's bar runs along the whole dock: {bar:?}"
    );
    for pane in ["content.1", "content.2"] {
        let body = harness.rect(harness.find(pane));
        assert!(
            body.bottom() <= bar.top() + 0.5,
            "{pane} gives the band its room: {body:?} above {bar:?}"
        );
    }
    let floating = harness.rect(harness.find("content.4"));
    assert!(
        floating.top() < bar.center().y,
        "a window placed past the band is moved up so its bar stays above the band: {floating:?}, {bar:?}"
    );

    layout.show(&3);
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("bar.9").is_some(),
        "the dock's bar shows whichever tab is shown"
    );
    layout.show(&2);
    harness.frame(Vec::new());
    assert!(harness.document().find_test_id("bar.9").is_some());

    layout.place_bar(&9, DockBarPlacement::Over(DockEdge::Top, DockAlign::End));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let bar = harness.rect(harness.find("bar.9"));
    let side = harness.rect(harness.find("content.2"));
    let near = WIDE_VIEWPORT.x - bar.right();
    assert!(
        near > 0.0 && near < 16.0 && bar.top() < 16.0,
        "floating over the top end, the bar sits at the dock's top right corner: {bar:?}"
    );
    assert!(
        bar.left() > side.left(),
        "it floats over whichever pane is at that corner: {bar:?} over {side:?}"
    );
    let stage = harness.rect(harness.find("content.3"));
    assert!(
        (stage.bottom() - WIDE_VIEWPORT.y).abs() < 0.5,
        "a floating bar takes no room"
    );

    let grip = harness.center(harness.find("dock.bar.9.grip"));
    harness.drag(grip, pos2(4.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Left)),
        "dragged to the dock's left edge, the bar docks beside it"
    );
    let bar = harness.rect(harness.find("bar.9"));
    let stage = harness.rect(harness.find("content.3"));
    assert!(
        stage.left() >= bar.right() - 0.5 && (bar.height() - WIDE_VIEWPORT.y).abs() < 0.5,
        "beside the left it runs down the whole dock: {bar:?}, {stage:?}"
    );

    let saved = layout.snapshot();
    layout.reset();
    harness.frame(Vec::new());
    assert_eq!(
        layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Bottom))
    );
    layout.restore(saved);
    harness.frame(Vec::new());
    assert_eq!(
        layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Left)),
        "the dock's bar is saved and restored with the layout"
    );
}
