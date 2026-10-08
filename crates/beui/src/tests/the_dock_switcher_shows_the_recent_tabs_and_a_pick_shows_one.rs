use super::*;
use crate::reactive::with_reactive_scope;
use crate::unstyled::{DockPane, DockSplit, DockTab, DockWindow, DockingLayout};
use crate::{Rect, vec2};

#[test]
fn the_dock_switcher_shows_the_recent_tabs_and_a_pick_shows_one() {
    let layout = DockingLayout::<u64>::new();
    let built = layout.clone();
    let document = build(move || {
        view! {
            <styled::Docking layout={built.clone()}>
                <DockSplit id="split">
                    <DockPane id="left">
                        <DockTab id=1u64 title="One">
                            <Frame />
                        </DockTab>
                        <DockTab id=2u64 title="Two">
                            <Frame />
                        </DockTab>
                    </DockPane>
                    <DockPane id="right">
                        <DockTab id=3u64 title="Three">
                            <Frame />
                        </DockTab>
                    </DockPane>
                </DockSplit>
                <DockWindow
                    id="window"
                    rect={Rect::from_min_size(pos2(40.0, 40.0), vec2(200.0, 150.0))}
                >
                    <DockPane id="window">
                        <DockTab id=4u64 title="Four">
                            <Frame />
                        </DockTab>
                    </DockPane>
                </DockWindow>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let shown = layout.clone();
    with_reactive_scope(harness.document_mut(), move || {
        for tab in [2, 4, 3, 1] {
            shown.show(&tab);
        }
    });
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.switch.1").is_none(),
        "the switcher is closed until a switch begins"
    );

    let switching = layout.clone();
    with_reactive_scope(harness.document_mut(), move || {
        switching.begin_switch(false);
        switching.step_switch(false);
    });
    harness.frame(Vec::new());
    for tab in 1..=4 {
        assert!(
            harness
                .document()
                .find_test_id(&format!("dock.switch.{tab}"))
                .is_some(),
            "the switcher lists tab {tab}, whichever pane or window it is in"
        );
    }
    assert_eq!(layout.switch_choice(), Some(4));
    assert_eq!(layout.focused(), Some(1), "switching shows nothing yet");

    let cancelling = layout.clone();
    with_reactive_scope(harness.document_mut(), move || cancelling.cancel_switch());
    harness.frame(Vec::new());
    assert!(harness.document().find_test_id("dock.switch.1").is_none());
    assert_eq!(layout.focused(), Some(1), "cancelling leaves the tab shown");

    let switching = layout.clone();
    with_reactive_scope(harness.document_mut(), move || switching.begin_switch(true));
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("dock.switch.4")));
    harness.frame(Vec::new());
    assert_eq!(layout.focused(), Some(4), "picking a row shows its tab");
    assert!(!layout.switching(), "and ends the switch");
}
