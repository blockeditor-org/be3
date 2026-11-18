use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[component]
fn LayerBar(name: &'static str) -> NodeId {
    let bar = dock::use_dock_bar();
    let direction = create_memo(move || bar.map_or(Direction::Horizontal, |bar| bar.direction()));
    view! {
        <Frame @test_id={format!("bar.{name}")}>
            <List direction={direction} spacing=4.0>
                <dock::styled::DockBarGrip />
                <Frame width=24.0 height=24.0 />
                <Frame width=24.0 height=24.0 />
            </List>
        </Frame>
    }
}

/// Three layers: the outer dock's tab 2 hosts the middle dock (panes 10 and 11), whose tab 11
/// hosts the inner dock (panes 20 over 21). The middle dock's own bar ("middle", lent to tab 2)
/// starts floating over its first pane; the inner dock's own bar ("inner", lent to tab 11) docks
/// in the top band of its first pane.
fn layered_dock() -> (Document, dock::DockingLayout<u64>) {
    use dock::{DockBar, DockPane, DockSplit, DockTab, DockingLayout, SplitSize};
    let built = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&built);
    let document = build_node(move || {
        let outer = DockingLayout::new();
        let middle = DockingLayout::<u64>::new();
        let inner = DockingLayout::<u64>::new();
        handle.set(Some(outer));
        let over = DockBarPlacement::Over(DockEdge::Top, DockAlign::Center);
        let band = DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center);
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout=outer focus=2u64>
                    <DockSplit id="root" size={SplitSize::Fraction(0.2)}>
                        <DockPane id="side">
                            <DockTab id=1u64 title="Side">
                                <Frame @test_id="content.1" />
                            </DockTab>
                        </DockPane>
                        <DockPane id="main">
                            <DockTab id=2u64 title="Nested">
                                <dock::Docking layout=middle>
                                    <DockSplit id="middle" size={SplitSize::Fraction(0.5)}>
                                        <DockPane id="left">
                                            <DockTab id=10u64 title="Left">
                                                <Frame @test_id="content.10" />
                                            </DockTab>
                                        </DockPane>
                                        <DockPane id="right">
                                            <DockTab id=11u64 title="Right">
                                                <dock::Docking layout=inner>
                                                    <DockSplit
                                                        id="inner"
                                                        size={SplitSize::Fraction(0.5)}
                                                        direction={Direction::Vertical}
                                                    >
                                                        <DockPane id="upper">
                                                            <DockTab id=20u64 title="Upper">
                                                                <Frame @test_id="content.20" />
                                                            </DockTab>
                                                        </DockPane>
                                                        <DockPane id="lower">
                                                            <DockTab id=21u64 title="Lower">
                                                                <Frame @test_id="content.21" />
                                                            </DockTab>
                                                        </DockPane>
                                                    </DockSplit>
                                                    <DockBar id=8u64 placement=band>
                                                        <LayerBar name="inner" />
                                                    </DockBar>
                                                </dock::Docking>
                                            </DockTab>
                                        </DockPane>
                                    </DockSplit>
                                    <DockBar id=9u64 placement=over>
                                        <LayerBar name="middle" />
                                    </DockBar>
                                </dock::Docking>
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    (document, built.get().expect("the dock was built"))
}

/// The grip leading a `LayerBar`.
fn grip_of(harness: &Harness, name: &str) -> Pos2 {
    let bar = harness.find(name);
    let list = harness.document().children(bar)[0];
    harness.center(harness.document().children(list)[0])
}

/// The middle layer's bar as the outer dock holds it (the one bar that may show on three
/// panes): the tab naming the pane it shows on, and its placement.
fn middle_bar(
    outer: &dock::DockingLayout<u64>,
) -> (Option<dock::TabId>, Option<DockBarPlacement>) {
    let state = outer.state().get_untracked();
    let bar = state
        .bars()
        .into_iter()
        .find(|bar| state.bar_panes(*bar).len() == 3)
        .expect("the middle dock's own bar is lent to the tab hosting it");
    (state.bar_tab(bar), state.bar_placement(bar))
}

fn inside(edge: DockEdge, body: Rect) -> Pos2 {
    match edge {
        DockEdge::Top => pos2(body.center().x, body.top() + 4.0),
        DockEdge::Bottom => pos2(body.center().x, body.bottom() - 4.0),
        DockEdge::Left => pos2(body.left() + 4.0, body.center().y),
        DockEdge::Right => pos2(body.right() - 4.0, body.center().y),
    }
}

fn along(edge: DockEdge, bar: Rect, body: Rect) -> bool {
    match edge {
        DockEdge::Top => (bar.bottom() - body.top()).abs() < 2.0,
        DockEdge::Bottom => (bar.top() - body.bottom()).abs() < 2.0,
        DockEdge::Left => (bar.right() - body.left()).abs() < 2.0,
        DockEdge::Right => (bar.left() - body.right()).abs() < 2.0,
    }
}

fn side_by_side(harness: &Harness, first: &str, second: &str) -> bool {
    let first = harness.rect(harness.find(first));
    let second = harness.rect(harness.find(second));
    (first.center().y - second.center().y).abs() < 2.0 && first.right() <= second.left() + 1.0
}

#[test]
fn a_joined_docks_bar_drops_on_any_edge_of_any_of_its_panes() {
    let _root = crate::reactive::RootScope::new();
    let (document, outer) = layered_dock();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let bar = harness.rect(harness.find("bar.middle"));
    let left = harness.rect(harness.find("content.10"));
    assert!(
        left.contains(bar.center()),
        "the middle layer's bar starts over its dock's first pane: {bar:?} in {left:?}"
    );

    // The bar docks on every edge of the middle layer's panes and of the inner layer's, which it
    // reaches through the dock the middle layer's second pane hosts. The top of the inner
    // layer's first pane, where its own bar is, comes last.
    for (pane, edge) in [
        ("content.10", DockEdge::Bottom),
        ("content.10", DockEdge::Left),
        ("content.10", DockEdge::Right),
        ("content.10", DockEdge::Top),
        ("content.21", DockEdge::Bottom),
        ("content.21", DockEdge::Left),
        ("content.21", DockEdge::Right),
        ("content.21", DockEdge::Top),
        ("content.20", DockEdge::Bottom),
        ("content.20", DockEdge::Left),
        ("content.20", DockEdge::Right),
    ] {
        let body = harness.rect(harness.find(pane));
        let grip = grip_of(&harness, "bar.middle");
        harness.drag_and_rest(grip, inside(edge, body));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let body = harness.rect(harness.find(pane));
        let bar = harness.rect(harness.find("bar.middle"));
        assert!(
            along(edge, bar, body),
            "dropped at the {edge:?} edge of {pane}, the bar docks beside it: {bar:?} by {body:?}, {:?}",
            middle_bar(&outer)
        );
        assert_eq!(
            middle_bar(&outer).1,
            Some(DockBarPlacement::Beside(edge, DockAlign::Center)),
            "at the {edge:?} edge of {pane}"
        );
    }

    // On the inner layer's pane, the middle layer's bar lands among the inner layer's bars where
    // it is dropped, and they among it.
    let inner = harness.rect(harness.find("bar.inner"));
    let grip = grip_of(&harness, "bar.middle");
    harness.drag_and_rest(grip, pos2(inner.right() + 30.0, inner.center().y));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        side_by_side(&harness, "bar.inner", "bar.middle"),
        "dropped after the inner layer's bar, the middle layer's bar lands after it: {:?}",
        middle_bar(&outer)
    );
    let middle = harness.rect(harness.find("bar.middle"));
    let grip = grip_of(&harness, "bar.inner");
    harness.drag_and_rest(grip, pos2(middle.right() + 30.0, middle.center().y));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        side_by_side(&harness, "bar.middle", "bar.inner"),
        "the inner layer's bar dropped after the middle layer's lands after it"
    );
}
