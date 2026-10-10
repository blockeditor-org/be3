use std::cell::RefCell;

use beui::ForwardedInput;
use beui::reactive::{Layers, component};

use super::*;

thread_local! {
    static HEARD: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
}

fn tab() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(SHOWN.x, 40.0))
}

#[component]
fn Covered(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    view! {
        <Layers>
            <Interactive
                focusable=true
                on_forward={|input: ForwardedInput| {
                    HEARD.with(|heard| heard.borrow_mut().extend(input.events));
                }}
            >
                <Frame width={Some(SCREEN.x)} height={Some(SCREEN.y)} />
            </Interactive>
            <List spacing=0.0>
                <ForEach keys={ids}>
                    {move |id: WindowId| {
                        let windows = windows.clone();
                        view! {
                            <Frame width={Some(SHOWN.x)} height={Some(SHOWN.y)}>
                                <WindowView windows id occluders={vec![tab()]} />
                            </Frame>
                        }
                    }}
                </ForEach>
            </List>
        </Layers>
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: beui::Modifiers::NONE,
    }
}

#[test]
fn a_drag_begun_on_the_ui_over_a_window_never_reaches_the_window() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Covered windows />
        }
    });
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let shown = harness.window(id);
    assert!(
        shown.contains(tab().center()),
        "the tab is drawn over the window"
    );
    harness.client.received.buttons.clear();

    let over = shown.center();
    harness.frame(vec![
        Event::PointerMoved(tab().center()),
        button(tab().center(), true),
    ]);
    harness.frame(vec![Event::PointerMoved(over)]);
    harness.frame(vec![button(over, false)]);
    harness.settle();
    assert!(
        harness.client.received.buttons.is_empty(),
        "a drag that began on the tab drawn over the window never reached the program: {:?}",
        harness.client.received.buttons
    );
    let heard = HEARD.with(|heard| heard.borrow().clone());
    assert!(
        heard.contains(&button(tab().center(), true)) && heard.contains(&button(over, false)),
        "the UI under the tab took the whole drag: {heard:?}"
    );

    harness.frame(vec![
        Event::PointerMoved(over),
        button(over, true),
        button(over, false),
    ]);
    harness.settle();
    assert_eq!(
        harness.client.received.buttons,
        vec![(BUTTON_LEFT, true), (BUTTON_LEFT, false)],
        "a press on the window's own uncovered part is the program's"
    );
}
