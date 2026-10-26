use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockTab, DockWindow, DockingLayout};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn closing_a_fullscreen_tab_leaves_the_tab_beside_it_in_its_window() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let layout = DockingLayout::<u64>::new();
    let setter = Rc::new(RefCell::new(None::<Signal<Vec<u64>>>));
    let document = {
        let (layout, setter) = (layout, setter.clone());
        build_node(move || {
            let keys = create_signal(vec![1u64, 2]);
            *setter.borrow_mut() = Some(keys);
            view! {
                <dock::styled::Docking @node_ref=&dock layout drag_modifier={Some(Modifiers::LOGO)}>
                    <DockPane
                        id="desktop"
                        empty={|| view! {
                            <Frame />
                        }}
                    />
                    <DockWindow
                        id="window"
                        rect={Rect::from_min_size(pos2(100.0, 80.0), vec2(360.0, 240.0))}
                    >
                        <DockPane id="program">
                            <ForEach keys>|id: u64|
                                <DockTab id title={format!("Tab {id}")}>
                                    <Panel id />
                                </DockTab>
                            </ForEach>
                        </DockPane>
                    </DockWindow>
                </dock::styled::Docking>
            }
        })
    };
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();

    layout.enter_fullscreen(&1, Some(Rect::from_min_size(Pos2::ZERO, WIDE_VIEWPORT)));
    harness.settle();
    assert_eq!(layout.fullscreen(), Some(1));

    let keys = setter.borrow_mut().take().expect("the tabs were built");
    keys.set(vec![2]);
    harness.settle();
    assert_eq!(
        layout.fullscreen(),
        None,
        "the closed tab is no longer fullscreen"
    );
    let state = harness.document().api::<DockApi>(dock).state();
    let window = state.windows()[0];
    let before = state.window_rect(window).expect("the window is open");
    let shown = harness.rect(harness.find("content.2"));
    assert!(
        before.contains_rect(shown),
        "the tab left in the window is shown inside it"
    );

    let held = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    drag_with(&mut harness, held, held + vec2(80.0, 50.0), Modifiers::LOGO);
    harness.settle();
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.window_rect(window),
        Some(before.translate(vec2(80.0, 50.0))),
        "the window followed the pointer"
    );
    assert_eq!(
        harness.rect(harness.find("content.2")),
        shown.translate(vec2(80.0, 50.0)),
        "and the tab's content went with it"
    );
}

#[component]
fn Panel(id: u64) -> NodeId {
    view! {
        <PlacementBuilder>|rect: ReadSignal<Rect>| {
            let anchor = create_memo(move || OverlayAnchor::Point(rect.get().min));
            view! {
                <Layers>
                    <Frame @test_id={format!("content.{id}")} />
                    <Overlay anchor traps_focus=false open=false>
                        <Frame width=40.0 height=20.0 />
                    </Overlay>
                </Layers>
            }
        }</PlacementBuilder>
    }
}
