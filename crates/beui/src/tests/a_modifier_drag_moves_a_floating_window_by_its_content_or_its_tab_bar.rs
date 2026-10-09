use super::*;
use crate::reactive::{NodeRef, build, view};
use crate::unstyled::dock_state;

#[test]
fn a_modifier_drag_moves_a_floating_window_by_its_content_or_its_tab_bar() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let layout = unstyled::DockingLayout::new();
        view! {
            <styled::Docking @node_ref=&built layout drag_modifier={Some(Modifiers::LOGO)}>
                <unstyled::DockPane
                    id="desktop"
                    empty={|| view! {
                        <Frame />
                    }}
                />
                <unstyled::DockWindow
                    id="window"
                    rect={Rect::from_min_size(pos2(100.0, 80.0), vec2(360.0, 240.0))}
                >
                    <unstyled::DockPane id="program">
                        <unstyled::DockTab id=1u64 title="Program">
                            <Frame @test_id="content" />
                        </unstyled::DockTab>
                        <unstyled::DockTab id=2u64 title="Other">
                            <Frame @test_id="other" />
                        </unstyled::DockTab>
                    </unstyled::DockPane>
                </unstyled::DockWindow>
            </styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let state = dock_state(harness.document(), dock);
    let window = state.windows()[0];
    let before = state.window_rect(window).expect("the window is open");
    let shown = harness.find("content");

    let held = harness.center(shown);
    drag_with(&mut harness, held, held + vec2(80.0, 50.0), Modifiers::LOGO);
    harness.frame(Vec::new());
    let state = dock_state(harness.document(), dock);
    assert_eq!(state.windows(), vec![window], "the window is the same one");
    assert_eq!(
        state.window_rect(window),
        Some(before.translate(vec2(80.0, 50.0))),
        "the whole window followed the pointer, keeping its size"
    );

    let tab = harness.center(dock_tab(harness.document(), dock, "Other"));
    drag_with(&mut harness, tab, tab + vec2(-40.0, 30.0), Modifiers::LOGO);
    harness.frame(Vec::new());
    let state = dock_state(harness.document(), dock);
    assert_eq!(
        state.window_rect(window),
        Some(before.translate(vec2(40.0, 80.0))),
        "a drag on a tab moves the window rather than the tab"
    );
    assert_eq!(
        state.surface_tabs(window).len(),
        2,
        "both tabs are still in the window"
    );
    assert_eq!(
        harness.find("content"),
        shown,
        "the tab that was shown still is"
    );
}
