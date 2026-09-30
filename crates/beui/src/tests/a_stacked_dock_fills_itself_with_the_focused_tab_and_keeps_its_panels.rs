use super::*;
use crate::reactive::with_reactive_scope;
use crate::unstyled::{DockMode, DockState, Side, TabId};

#[test]
fn a_stacked_dock_fills_itself_with_the_focused_tab_and_keeps_its_panels() {
    let (mode, set_mode) = create_signal(DockMode::Stacked);
    let document = build(move || {
        let mut layout = DockState::new([TabId::new(1), TabId::new(2)]);
        let leaf = layout.leaves(layout.main())[0];
        layout.split(leaf, Side::Right, 0.5, vec![TabId::new(3)]);
        let (state, set_state) = create_signal(layout);
        view! {
            <styled::DockArea
                state={state}
                mode={mode}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| set_state.set(next)}
                on_close={move |_: TabId| {}}
            >
                {move |tab: TabId| view! {
                    <Frame @test_id={format!("content.{}", tab.value())} />
                }}
            </styled::DockArea>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let third = harness
        .document()
        .find_test_id("content.3")
        .expect("the stacked dock shows the focused tab");
    let bar = harness.rect(harness.find("dock.switch"));
    assert_eq!(
        harness.rect(third),
        Rect::from_min_max(
            pos2(0.0, harness.rect(third).top()),
            pos2(WIDE_VIEWPORT.x, WIDE_VIEWPORT.y)
        ),
        "the focused tab fills the dock below its bar"
    );
    assert!(harness.rect(third).top() >= bar.bottom());
    assert!(
        harness.document().find_test_id("content.1").is_none(),
        "the pane beside it is not drawn"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.3.close")
            .is_none(),
        "a stacked dock draws no tab bars"
    );

    let tiling = set_mode.clone();
    with_reactive_scope(harness.document_mut(), move || tiling.set(DockMode::Tiled));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().find_test_id("content.3"),
        Some(third),
        "tiling the dock again keeps the panel it was showing"
    );
    assert!(
        harness.rect(third).left() > WIDE_VIEWPORT.x / 3.0,
        "the tab is back in the pane it was split into"
    );
    harness.find("content.1");

    with_reactive_scope(harness.document_mut(), move || {
        set_mode.set(DockMode::Stacked)
    });
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().find_test_id("content.3"),
        Some(third),
        "stacking it again shows the same panel"
    );
    assert_eq!(
        harness.rect(third).width(),
        WIDE_VIEWPORT.x,
        "stacking it again fills the dock with the same panel"
    );
}
