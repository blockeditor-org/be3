use super::*;
use crate::dock::{DockPane, DockTab, DockTabControl, DockingLayout, use_dock_tab};
use crate::reactive::*;

/// A view of a fullscreen tab's content that takes the keyboard counts, so leaving fullscreen and
/// showing another tab hands that keyboard on.
#[test]
fn a_fullscreen_tabs_content_taking_the_keyboard_counts() {
    let _root = crate::reactive::RootScope::new();
    let controls = create_signal(Vec::<(u64, DockTabControl)>::new());
    let layout = DockingLayout::<u64>::new();
    let document = build_node(move || {
        let tab = move |id: u64| {
            view! {
                <DockTab
                    id
                    title={format!("Tab {id}")}
                    content={move || {
                        if let Some(control) = use_dock_tab() {
                            controls.update(|controls| controls.push((id, control)));
                        }
                        view! {
                            <Frame />
                        }
                    }}
                />
            }
        };
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout focus=1u64>
                    <DockPane id="tabs">{tab(1)}{tab(2)}</DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    layout.show(&2);
    harness.settle();
    layout.show(&1);
    harness.settle();
    let control = |id: u64| {
        controls
            .get_untracked()
            .into_iter()
            .find(|(held, _)| *held == id)
            .map(|(_, control)| control)
            .expect("each tab's content has its control")
    };
    layout.enter_fullscreen(&1, None);
    harness.settle();
    harness.frame(vec![Input::Focus(false)]);
    control(1).content_focused();
    harness.settle();
    layout.leave_fullscreen();
    harness.settle();
    let before = control(2).keyboard();
    layout.show(&2);
    harness.settle();
    assert_ne!(
        control(2).keyboard(),
        before,
        "the keyboard the fullscreen tab's content took goes to the tab shown in its place"
    );
}
