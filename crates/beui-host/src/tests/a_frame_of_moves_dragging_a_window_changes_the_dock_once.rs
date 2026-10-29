use super::*;
use crate::dock::unstyled::{DockApi, DockingLayout};
use crate::reactive::*;

#[test]
fn a_frame_of_moves_dragging_a_window_changes_the_dock_once() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let layout = DockingLayout::<u64>::new();
    let document = build_node(move || {
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <dock::unstyled::DockPane id="desktop" />
                <dock::unstyled::DockWindow
                    id="window"
                    rect={Rect::from_min_size(pos2(100.0, 80.0), vec2(360.0, 240.0))}
                >
                    <dock::unstyled::DockPane id="program">
                        <dock::unstyled::DockTab id=1u64 title="Program">
                            <Frame />
                        </dock::unstyled::DockTab>
                    </dock::unstyled::DockPane>
                </dock::unstyled::DockWindow>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    let window = state.windows()[0];
    let before = state.window_rect(window).expect("the window is open");
    let tab = harness
        .document()
        .node_rect(dock_tab(harness.document(), dock, "Program"))
        .expect("the tab is laid out");
    let held = pos2(tab.max.x + 40.0, tab.center().y);
    let changes = Rc::new(std::cell::Cell::new(0));
    let counted = Rc::clone(&changes);
    create_effect(move || {
        layout.state().with(|_| ());
        counted.set(counted.get() + 1);
    });

    harness.frame(vec![
        Input::PointerMoved(held),
        Input::PointerButton {
            pos: held,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    harness.frame(Vec::new());
    let settled = changes.get();
    harness.frame(
        (1..=8)
            .map(|step| Input::PointerMoved(held + vec2(10.0 * step as f32, 5.0 * step as f32)))
            .collect(),
    );
    assert_eq!(
        changes.get() - settled,
        1,
        "every move a frame brings is taken into one change of the dock, not one change each"
    );
    harness.frame(vec![Input::PointerButton {
        pos: held + vec2(80.0, 40.0),
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.window_rect(window),
        Some(before.translate(vec2(80.0, 40.0))),
        "the window ends under the last move"
    );
}
