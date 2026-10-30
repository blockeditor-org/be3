use super::*;
use crate::dock::unstyled::{
    DockAlign,    DockBar, DockBarPlacement, DockEdge, DockPane, DockSplit, DockTab, DockingLayout, TabId,
    use_dock_bar,
};

#[component]
fn SharedTools(id: u64, tools: usize, pressed: Signal<Vec<u64>>) -> NodeId {
    let bar = use_dock_bar();
    let direction = create_memo(move || bar.map_or(Direction::Horizontal, |bar| bar.direction()));
    view! {
        <Frame @test_id={format!("bar.{id}")}>
            <List direction={direction} spacing=4.0>
                <Frame @test_id={format!("grip.{id}")}>
                    <dock::styled::DockBarGrip />
                </Frame>
                <ForEach keys={(0..tools).collect::<Vec<_>>()}>|tool: usize|
                    <Interactive
                        on_press={move |_: PointerPress| pressed.update(|pressed| pressed.push(id))}
                    >
                        <Frame @test_id={format!("tool.{id}.{tool}")} width=24.0 height=24.0 />
                    </Interactive>
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn SharedCanvas(id: u64, tools: usize, pressed: Signal<Vec<u64>>) -> NodeId {
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <dock::styled::Docking layout>
            <DockPane id="stage" bare=true>
                <DockTab id="stage" title="Stage">
                    <Frame @test_id={format!("content.{id}")} />
                </DockTab>
            </DockPane>
            <DockBar id="tools" parent="stage" placement={DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center)}>
                <SharedTools id tools pressed />
            </DockBar>
        </dock::styled::Docking>
    }
}

fn pick(harness: &mut Harness, grip: &str, row: &str) {
    harness.click(harness.center(harness.find(grip)));
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the document has a root");
    let row = text_within(harness.document(), root, row).expect("the grip opens its menu");
    harness.click(harness.center(row));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
}

#[test]
fn bars_of_one_key_from_two_docked_tabs_share_one_window_that_follows_the_tab_shown_last() {
    let _root = crate::reactive::RootScope::new();
    let built = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&built);
    let document = build_node(move || {
        let outer = DockingLayout::<u64>::new();
        let pressed = create_signal(Vec::new());
        handle.set(Some((outer, pressed)));
        view! {
            <dock::styled::Docking layout=outer focus=2u64>
                <DockSplit id="root" fraction=0.7>
                    <DockPane id="canvases">
                        <DockTab id=1u64 title="Canvas 1">
                            <SharedCanvas id=1 tools=2 pressed />
                        </DockTab>
                        <DockTab id=2u64 title="Canvas 2">
                            <SharedCanvas id=2 tools=5 pressed />
                        </DockTab>
                        <DockTab id=3u64 title="Notes">
                            <Frame @test_id="content.3" />
                        </DockTab>
                    </DockPane>
                    <DockPane id="side">
                        <DockTab id=4u64 title="Side">
                            <Frame @test_id="content.4" />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let (outer, pressed) = built.get().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    outer.show(&1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let placements = || {
        let state = outer.state().get_untracked();
        state
            .bars()
            .into_iter()
            .map(|bar| state.bar_placement(bar))
            .collect::<Vec<_>>()
    };
    assert_eq!(placements().len(), 2, "both docked tabs declared their bar");

    pick(&mut harness, "grip.1", "Pop out into a window");
    let popped = placements();
    assert!(
        popped
            .iter()
            .all(|placement| placement.is_some_and(DockBarPlacement::is_window)),
        "popping out one tab's bar pops out every bar of its key: {popped:?}"
    );
    assert_eq!(popped[0], popped[1], "they share one place");
    let first = harness.rect(harness.find("bar.1"));

    outer.show(&2);
    harness.frame(Vec::new());
    let second = harness.rect(harness.find("bar.2"));
    assert_eq!(
        second.min, first.min,
        "the window shows the bar of the tab shown last, from the same corner"
    );
    assert!(
        second.width() > first.width(),
        "and takes that bar's size: {second:?} after {first:?}"
    );
    harness.click(harness.center(harness.find("tool.2.4")));
    harness.frame(Vec::new());
    assert_eq!(
        pressed.get_untracked(),
        vec![2],
        "a press in the window reaches the bar it shows"
    );

    outer.show(&4);
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("tool.2.0")));
    harness.frame(Vec::new());
    assert_eq!(pressed.get_untracked(), vec![2, 2]);
    let state = outer.state().get_untracked();
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(4)),
        "pressing the window does not count as showing the tab whose bar it shows"
    );
    assert!(harness.document().find_test_id("bar.2").is_some());

    outer.show(&3);
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("bar.1").is_none()
            && harness.document().find_test_id("bar.2").is_none(),
        "while neither tab with the bar is shown, the window hides"
    );

    outer.show(&1);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(harness.find("bar.1")).min, first.min);
    harness.click(harness.center(harness.find("tool.1.1")));
    harness.frame(Vec::new());
    assert_eq!(pressed.get_untracked(), vec![2, 2, 1]);

    pick(&mut harness, "grip.1", "Float over the bottom");
    assert_eq!(
        placements(),
        vec![Some(DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center)); 2],
        "docking one back docks them all"
    );
}
