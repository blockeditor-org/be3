use super::*;
use crate::dock::{DockPane, DockTab, DockTabControl, DockingLayout, use_dock_tab};
use crate::primitives::*;
use crate::reactive::*;

/// A tab whose content took the keyboard in a view this document cannot see (a child editor's)
/// hands it to the tab shown in its place, but only while nothing else has taken it since.
#[test]
fn a_stacked_dock_hands_on_the_keyboard_a_tabs_content_had_until_something_else_takes_it() {
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
                <dock::Docking layout focus=2u64>
                    <DockPane id="tabs">{tab(1)}{tab(2)}</DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, PHONE_VIEWPORT);
    harness.settle();
    layout.show(&1);
    harness.settle();
    layout.show(&2);
    harness.settle();
    let control = |id: u64| {
        controls
            .get_untracked()
            .into_iter()
            .find(|(held, _)| *held == id)
            .map(|(_, control)| control)
            .expect("each tab's content has its control")
    };

    harness.frame(vec![Input::Focus(false)]);
    control(2).content_focused();
    harness.settle();
    let before = control(1).keyboard();
    layout.show(&1);
    harness.settle();
    assert_ne!(
        control(1).keyboard(),
        before,
        "the keyboard tab 2's content had goes to tab 1, shown in its place"
    );

    layout.show(&2);
    harness.settle();
    control(2).content_focused();
    harness.settle();
    harness.frame(vec![Input::Focus(true)]);
    harness.frame(vec![Input::Focus(false)]);
    harness.settle();
    let before = control(1).keyboard();
    layout.show(&1);
    harness.settle();
    assert_eq!(
        control(1).keyboard(),
        before,
        "the document took the keyboard since, so hiding tab 2 hands on nothing"
    );
}
