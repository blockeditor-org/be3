use std::time::Duration;

use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::MenuItem;

const LONG_PRESS: Duration = Duration::from_millis(500);

fn sheet_shown(harness: &Harness) -> bool {
    harness
        .document()
        .find_test_id("sheet.handle")
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn holding_a_finger_down_opens_a_context_menu() {
    let _root = crate::reactive::RootScope::new();
    let menu = NodeRef::new();
    let clicked = create_signal(0);
    let cancelled = create_signal(0);
    let document = build_node(move || {
        view! {
            <ContextMenu
                @node_ref=&menu
                items={view! {
                    <MenuItem label="Copy" />
                }}
            >
                <Interactive
                    on_click={move || clicked.update(|clicked| *clicked += 1)}
                    on_cancel={move || cancelled.update(|cancelled| *cancelled += 1)}
                >
                    <Frame width=300.0 height=300.0 />
                </Interactive>
            </ContextMenu>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.touch(TouchPhase::Start, pos2(100.0, 100.0));
    let waiting = harness.frame(Vec::new());
    assert!(
        waiting.repaint_after > Duration::ZERO && waiting.repaint_after <= LONG_PRESS,
        "a finger held down asks to be woken when it becomes a long press"
    );
    harness.touch(TouchPhase::End, pos2(100.0, 100.0));
    assert_eq!(clicked.get_untracked(), 1, "a tap is a click");
    assert!(!sheet_shown(&harness));

    harness.context().set_long_press_delay(Duration::ZERO);
    harness.touch(TouchPhase::Start, pos2(100.0, 100.0));
    harness.frame(Vec::new());
    assert!(
        sheet_shown(&harness),
        "holding a finger still opens the menu, as a sheet"
    );
    assert!(
        !harness
            .document()
            .api::<unstyled::ContextMenuApi>(menu.get())
            .open(harness.document()),
        "a finger never opens the menu that a mouse gets"
    );
    assert_eq!(
        cancelled.get_untracked(),
        1,
        "the press under the finger is called off"
    );
    harness.touch(TouchPhase::End, pos2(100.0, 100.0));
    harness.frame(Vec::new());

    assert_eq!(
        clicked.get_untracked(),
        1,
        "lifting the finger is not a click"
    );
    assert!(
        sheet_shown(&harness),
        "lifting the finger leaves the menu open"
    );

    harness.touch(TouchPhase::Start, pos2(100.0, 100.0));
    harness.touch(TouchPhase::Move, pos2(100.0, 180.0));
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, pos2(100.0, 180.0));
    assert_eq!(
        cancelled.get_untracked(),
        1,
        "a finger that moved is a drag, not a hold"
    );
}
