use std::time::Duration;

use beui_types::Rect;
use beui_types::{Aim, Replay, Replayed, pos2, vec2};
use beui_types::{
    Input, Key, KeyPress, Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId,
    TouchPhase,
};
use beui_types::{PointerEvent, PointerPhase};

use super::*;

mod a_held_press_is_replayed_where_the_pointer_is_and_leaves_once_over_nothing;
mod a_mouse_press_the_host_gave_up_is_cancelled;
mod a_move_carries_its_prediction_along;
mod a_scroll_is_replayed_in_unzoomed_pixels;
mod an_unnamed_key_is_replayed_as_a_keystroke;
mod focus_is_replayed_once_for_each_change;
mod modifiers_are_replayed_only_when_they_change;
mod pen_phases_become_pen_inputs;
mod pointer_inputs_keep_their_time;
mod pointers_are_replayed_from_the_region_corner_and_unzoomed;
mod touch_phases_become_touch_inputs;

const PRIMARY: PointerButton = PointerButton::Primary;

fn rect() -> Rect {
    Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 200.0))
}

fn pointer(pointer: PointerId, phase: PointerPhase, pos: Pos2) -> PointerEvent {
    PointerEvent {
        pointer,
        window: beui_types::WindowId::MAIN,
        phase,
        pos,
        modifiers: Modifiers::NONE,
        clicks: 0,
        force: None,
        pen: None,
        predicted: None,
        time: crate::timer::now(),
        source: 0,
    }
}

fn sent(event: PointerEvent) -> Forwarded {
    Forwarded::Pointer { event, over: None }
}

fn mouse(phase: PointerPhase, pos: Pos2) -> Forwarded {
    sent(pointer(PointerId::Mouse, phase, pos))
}

fn replayed(replay: &mut Replay, events: Vec<Forwarded>, zoom: f32) -> Vec<Input> {
    timed(replay, events, zoom)
        .into_iter()
        .map(|replayed| replayed.input)
        .collect()
}

fn timed(replay: &mut Replay, events: Vec<Forwarded>, zoom: f32) -> Vec<Replayed> {
    let input = ForwardedInput {
        events,
        rect: rect(),
        hovered: true,
        focused: true,
        pointer: None,
        modifiers: Modifiers::NONE,
    };
    replay.replay(&input, zoom, |_, _| Aim::Own)
}

fn button(pos: Pos2, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button: PRIMARY,
        pressed,
    }
}

fn key(key: Key, modifiers: Modifiers, text: Option<&str>) -> Forwarded {
    Forwarded::Input(Event::Key {
        press: KeyPress {
            key,
            pressed: true,
            repeat: false,
            modifiers,
        },
        evdev: Some(30),
        text: text.map(str::to_owned),
    })
}
