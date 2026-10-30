use super::*;
use crate::dock::unstyled::{
    DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockTab, DockingLayout,
};

#[component]
fn BandBar(id: u64, thickness: f32) -> NodeId {
    view! {
        <Frame @test_id={format!("bar.{id}")}>
            <List direction=Direction::Horizontal spacing=4.0>
                <dock::styled::DockBarGrip />
                <Frame width=40.0 height=thickness />
            </List>
        </Frame>
    }
}

fn top(align: DockAlign) -> DockBarPlacement {
    DockBarPlacement::Beside(DockEdge::Top, align)
}

#[test]
fn bars_docked_along_one_edge_share_its_band_side_by_side() {
    let _root = crate::reactive::RootScope::new();
    let built = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&built);
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        handle.set(Some(layout));
        view! {
            <dock::styled::Docking layout focus=1u64>
                <DockPane id="stage" bare=true>
                    <DockTab id=1u64 title="Stage">
                        <Frame @test_id="content.1" />
                    </DockTab>
                </DockPane>
                <DockBar id=5u64 parent=1u64 placement={top(DockAlign::Start)}>
                    <BandBar id=5u64 thickness=24.0 />
                </DockBar>
                <DockBar id=6u64 parent=1u64 placement={top(DockAlign::Start)}>
                    <BandBar id=6u64 thickness=24.0 />
                </DockBar>
                <DockBar id=7u64 parent=1u64 placement={top(DockAlign::Center)}>
                    <BandBar id=7u64 thickness=24.0 />
                </DockBar>
                <DockBar id=8u64 parent=1u64 placement={top(DockAlign::End)}>
                    <BandBar id=8u64 thickness=40.0 />
                </DockBar>
            </dock::styled::Docking>
        }
    });
    let layout = built.get().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let rect = |harness: &Harness, id: &str| harness.rect(harness.find(id));
    let near = |first: f32, second: f32| (first - second).abs() < 0.5;

    let band = rect(&harness, "dock.bar.band.top");
    let body = rect(&harness, "content.1");
    let [five, six, seven, eight] =
        ["bar.5", "bar.6", "bar.7", "bar.8"].map(|id| rect(&harness, id));
    assert!(
        body.top() >= band.bottom() - 0.5,
        "the body gives the one band its room: {body:?} under {band:?}"
    );
    for bar in [five, six, seven, eight] {
        assert!(
            band.contains_rect(bar),
            "every bar beside the top shares its one band: {bar:?} in {band:?}"
        );
    }
    assert!(
        near(band.height(), eight.height()) || band.height() > eight.height(),
        "the band is as thick as its thickest bar: {band:?}, {eight:?}"
    );
    assert!(
        band.height() < eight.height() * 1.5,
        "and no thicker than it needs: {band:?}, {eight:?}"
    );
    assert!(
        near(five.left(), band.left()) && near(six.left(), five.right()),
        "two bars at the start line up along the edge, the second right after the first: {five:?}, {six:?}"
    );
    assert!(
        near(seven.center().x, band.center().x),
        "the centred bar sits at the band's centre: {seven:?} in {band:?}"
    );
    assert!(
        near(eight.right(), band.right()),
        "the bar at the end sits at the band's end: {eight:?} in {band:?}"
    );

    let grip = harness.center(harness.find("dock.bar.6.grip"));
    harness.drag(grip, pos2(five.left() + 20.0, band.top() + 4.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(layout.bar_placement(&6), Some(top(DockAlign::Start)));
    let [five, six] = ["bar.5", "bar.6"].map(|id| rect(&harness, id));
    assert!(
        near(six.left(), band.left()) && near(five.left(), six.right()),
        "dropped over the start of the first bar, the bar slots in before it: {six:?}, {five:?}"
    );

    let grip = harness.center(harness.find("dock.bar.7.grip"));
    harness.drag(grip, pos2(eight.right() - 10.0, band.top() + 4.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        layout.bar_placement(&7),
        Some(top(DockAlign::End)),
        "dropped over the bar at the end, the bar takes its alignment"
    );
    let [seven, eight] = ["bar.7", "bar.8"].map(|id| rect(&harness, id));
    assert!(
        near(seven.right(), band.right()) && near(eight.right(), seven.left()),
        "and slots in after it, past its middle: {eight:?}, {seven:?}"
    );
}
