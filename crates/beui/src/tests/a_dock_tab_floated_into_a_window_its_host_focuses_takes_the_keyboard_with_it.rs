use super::*;
use crate::{WindowId, WindowReport, WindowRequest};

#[test]
fn a_dock_tab_floated_into_a_window_its_host_focuses_takes_the_keyboard_with_it() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let first = create_signal(String::new());
    let other = create_signal(String::new());
    let document = build_node(move || {
        let layout = dock::unstyled::DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <dock::unstyled::DockSplit id="split">
                    <dock::unstyled::DockPane id="left">
                        <dock::unstyled::DockTab id=1u64 title="Tab 1">
                            <styled::TextInput @test_id="first" value={first} label="First" />
                        </dock::unstyled::DockTab>
                        <dock::unstyled::DockTab id=3u64 title="Tab 3">
                            <styled::TextInput @test_id="other" value={other} label="Other" />
                        </dock::unstyled::DockTab>
                    </dock::unstyled::DockPane>
                    <dock::unstyled::DockPane id="right">
                        <dock::unstyled::DockTab id=2u64 title="Tab 2">
                            <Frame />
                        </dock::unstyled::DockTab>
                    </dock::unstyled::DockPane>
                </dock::unstyled::DockSplit>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    press_in(&mut harness, WindowId::MAIN, tab, true);
    press_in(&mut harness, WindowId::MAIN, tab, false);
    let input = harness.find("first");
    let at = harness.rect(input).center();
    press_in(&mut harness, WindowId::MAIN, at, true);
    press_in(&mut harness, WindowId::MAIN, at, false);
    assert!(harness.document().focus_is_within(input));
    harness.take_window_requests();

    press_in(&mut harness, WindowId::MAIN, tab, true);
    harness.frame(vec![Input::PointerMoved(tab + vec2(30.0, 30.0))]);
    harness.frame(vec![Input::PointerLeft]);
    let nowhere = pos2(-60.0, -40.0);
    harness.frame(vec![Input::PointerMoved(nowhere)]);
    press_in(&mut harness, WindowId::MAIN, nowhere, false);
    harness.frame(Vec::new());
    let window = window_showing(&harness, dock, "Tab 1");
    assert!(
        harness
            .take_window_requests()
            .iter()
            .any(|request| matches!(request, WindowRequest::Open { window: opened, .. } if *opened == window)),
        "floating the tab opens a window"
    );

    harness.report_window(WindowReport::Focused {
        window: WindowId::MAIN,
        focused: false,
    });
    harness.report_window(WindowReport::Focused {
        window,
        focused: true,
    });
    harness.frame(Vec::new());
    let input = harness.find("first");
    assert!(
        harness.document().focus_is_within(input),
        "the window the host focuses has the keyboard, in the tab it holds"
    );
    harness.type_text("typed");
    harness.frame(Vec::new());
    assert_eq!(first.get_untracked(), "typed");
    assert_eq!(
        other.get_untracked(),
        "",
        "nothing left in the dock gets it"
    );
}
