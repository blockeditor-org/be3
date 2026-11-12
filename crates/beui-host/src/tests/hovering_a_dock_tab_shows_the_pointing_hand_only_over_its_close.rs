use super::*;
use crate::primitives::Frame;
use beui::CursorIcon;

#[test]
fn hovering_a_dock_tab_shows_the_pointing_hand_only_over_its_close() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let layout = dock::DockingLayout::<&'static str>::new();
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout>
                    <dock::DockPane id="side">
                        <dock::DockTab id="notes" title="Notes" on_close={|| {}}>
                            <Frame />
                        </dock::DockTab>
                    </dock::DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let tab = harness.rect(harness.find("dock.tab.notes"));
    let close = harness.rect(harness.find("dock.tab.notes.close"));

    let output = harness.frame(vec![Input::PointerMoved(Pos2::new(
        tab.min.x + 4.0,
        tab.center().y,
    ))]);
    assert_eq!(
        output.main().window.requests.cursor,
        CursorIcon::Default,
        "a tab is not a link: it keeps the default cursor"
    );

    let output = harness.frame(vec![Input::PointerMoved(close.center())]);
    assert_eq!(
        output.main().window.requests.cursor,
        CursorIcon::PointingHand,
        "the tab's close button is a button"
    );
}
