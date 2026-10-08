use super::*;
use crate::reactive::{NodeRef, build, view};
use crate::unstyled::dock_state;

#[test]
fn a_window_carried_by_its_content_over_an_empty_pane_moves_as_it_was_held() {
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

    let held = harness.center(harness.find("content"));
    drag_with(&mut harness, held, held + vec2(80.0, 50.0), Modifiers::LOGO);
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    assert_eq!(state.windows(), vec![window], "the window is the same one");
    assert_eq!(
        state.window_rect(window),
        Some(before.translate(vec2(80.0, 50.0))),
        "it moved with the pointer, keeping its size and where it was held"
    );
}
