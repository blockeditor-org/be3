use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use super::*;
use crate::reactive::{ClickCatcher, Frame, view};
use crate::styled::ContextMenu;
use crate::unstyled::MenuItem;

const LONG_PRESS: Duration = Duration::from_millis(500);

#[test]
fn holding_a_finger_down_opens_a_context_menu() {
    let menu = NodeRef::new();
    let clicked = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let document = crate::reactive::build({
        let (menu, clicked, cancelled) = (menu.clone(), clicked.clone(), cancelled.clone());
        move || {
            view! {
                <ContextMenu
                    @node_ref=&menu
                    items={view! {
                        <MenuItem label="Copy" />
                    }}
                >
                    <ClickCatcher
                        on_click={move || clicked.set(clicked.get() + 1)}
                        on_cancel={move || cancelled.set(cancelled.get() + 1)}
                    >
                        <Frame width=300.0 height=300.0 />
                    </ClickCatcher>
                </ContextMenu>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let overlay = unstyled::context_menu_overlay(harness.document(), menu.get());

    harness.touch(TouchPhase::Start, pos2(100.0, 100.0));
    let waiting = harness.frame(Vec::new());
    assert!(
        waiting.repaint_after > Duration::ZERO && waiting.repaint_after <= LONG_PRESS,
        "a finger held down asks to be woken when it becomes a long press"
    );
    harness.touch(TouchPhase::End, pos2(100.0, 100.0));
    assert_eq!(clicked.get(), 1, "a tap is a click");
    assert!(!harness.document().is_overlay_open(overlay));

    harness.context.set_long_press_delay(Duration::ZERO);
    harness.touch(TouchPhase::Start, pos2(100.0, 100.0));
    harness.frame(Vec::new());
    assert!(
        harness.document().is_overlay_open(overlay),
        "holding a finger still opens the menu"
    );
    assert_eq!(
        cancelled.get(),
        1,
        "the press under the finger is called off"
    );
    harness.touch(TouchPhase::End, pos2(100.0, 100.0));
    harness.frame(Vec::new());

    assert_eq!(clicked.get(), 1, "lifting the finger is not a click");
    assert!(
        harness.document().is_overlay_open(overlay),
        "lifting the finger leaves the menu open"
    );

    harness.touch(TouchPhase::Start, pos2(100.0, 100.0));
    harness.touch(TouchPhase::Move, pos2(100.0, 180.0));
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, pos2(100.0, 180.0));
    assert_eq!(
        cancelled.get(),
        1,
        "a finger that moved is a drag, not a hold"
    );
}
