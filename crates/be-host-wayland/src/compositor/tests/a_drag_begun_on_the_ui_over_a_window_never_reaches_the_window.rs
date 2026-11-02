use std::cell::RefCell;

use beui::styled::*;
use beui::{ForwardedInput, PointerPhase};

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
                    let buttons = input.pointer_events().filter_map(|event| match event.phase {
                        PointerPhase::Down(button) | PointerPhase::Up(button) => {
                            Some(Event::PointerButton {
                                pos: event.pos,
                                button,
                                pressed: matches!(event.phase, PointerPhase::Down(_)),
                                modifiers: event.modifiers,
                            })
                        }
                        _ => None,
                    });
                    HEARD.with(|heard| heard.borrow_mut().extend(buttons));
                }}
            >
                <Frame width={Size::Custom(SCREEN.x)} height={Size::Custom(SCREEN.y)} />
            </Interactive>
            <List spacing=Space::None>
                <ForEach keys={ids}>|id: WindowId|
                    <Frame width={Size::Custom(SHOWN.x)} height={Size::Custom(SHOWN.y)}>
                        <WindowView windows id occluders={vec![tab()]} />
                    </Frame>
                </ForEach>
            </List>
        </Layers>
    }
}

fn button(pos: Pos2, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    }
}

fn heard_button(pos: Pos2, pressed: bool) -> Event {
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
        Input::PointerMoved(tab().center()),
        button(tab().center(), true),
    ]);
    harness.frame(vec![Input::PointerMoved(over)]);
    harness.frame(vec![button(over, false)]);
    harness.settle();
    assert!(
        harness.client.received.buttons.is_empty(),
        "a drag that began on the tab drawn over the window never reached the program: {:?}",
        harness.client.received.buttons
    );
    let heard = HEARD.with(|heard| heard.borrow().clone());
    assert!(
        heard.contains(&heard_button(tab().center(), true))
            && heard.contains(&heard_button(over, false)),
        "the UI under the tab took the whole drag: {heard:?}"
    );

    harness.frame(vec![
        Input::PointerMoved(over),
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
