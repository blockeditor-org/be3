use super::*;
use crate::dock::unstyled::{DockApi, DockTabControl, use_dock_tab};
use crate::{WindowReport, WindowRequest};

#[test]
fn a_hosted_dock_window_resized_by_its_tab_asks_its_host_and_takes_the_size_it_reports() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let control = StoredCell::new(None::<DockTabControl>);
    let document = build_node(move || {
        let layout = dock::unstyled::DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <dock::unstyled::DockPane id="base">
                    <dock::unstyled::DockTab id=1u64 title="Base">
                        <Frame />
                    </dock::unstyled::DockTab>
                </dock::unstyled::DockPane>
                <dock::unstyled::DockWindow
                    id="window"
                    rect={Rect::from_min_size(pos2(60.0, 60.0), vec2(360.0, 240.0))}
                >
                    <dock::unstyled::DockPane id="floating">
                        <dock::unstyled::DockTab id=2u64 title="Floating">
                            <Grabbing control />
                        </dock::unstyled::DockTab>
                    </dock::unstyled::DockPane>
                </dock::unstyled::DockWindow>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let window = window_showing(&harness, dock, "Floating");
    let opened = harness
        .windows()
        .window(window)
        .expect("the host opened the window")
        .size;
    let grips = opened - vec2(360.0, 240.0);
    harness.take_window_requests();

    control
        .get()
        .expect("the tab has its dock's control")
        .resize_window(vec2(300.0, 200.0));
    harness.frame(Vec::new());
    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::Resize {
                window,
                size: vec2(300.0, 200.0) + grips,
            }),
        "the window asks its host for the new size"
    );

    harness.report_window(WindowReport::Resized {
        window,
        size: vec2(320.0, 220.0) + grips,
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    let surface = state.windows()[0];
    assert_eq!(
        state.window_rect(surface).map(|rect| rect.size()),
        Some(vec2(320.0, 220.0)),
        "the dock keeps the size its host gave"
    );
    assert_eq!(
        harness.windows().output(window).map(|output| output.size),
        Some(vec2(320.0, 220.0) + grips),
        "and the window is painted at it"
    );
    assert!(
        harness.take_window_requests().is_empty(),
        "following the host's size asks for nothing more"
    );
}

#[component]
fn Grabbing(control: StoredCell<Option<DockTabControl>>) -> NodeId {
    control.set(use_dock_tab());
    view! {
        <Frame />
    }
}
