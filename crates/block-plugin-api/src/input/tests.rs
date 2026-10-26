use super::*;
use crate::{Modifiers, PointerButton, Pos2, ScrollDelta, Vec2};

fn input(event: impl Into<TimedInput>) -> InputBatch {
    batch(vec![event.into()], false)
}

fn timed(input: Input, time_micros: u64) -> TimedInput {
    TimedInput {
        input,
        time_micros: Some(time_micros),
        over: None,
    }
}

fn batch(events: Vec<TimedInput>, every_sample: bool) -> InputBatch {
    InputBatch {
        screen: crate::ScreenId(7),
        events,
        every_sample,
    }
}

fn pen(x: f32, time_micros: u64) -> TimedInput {
    timed(
        Input::Pen {
            phase: crate::PenPhase::Move,
            pos: crate::Pos2::new(x, 0.0),
            pen: crate::Pen {
                pressure: 0.5,
                ..crate::Pen::default()
            },
        },
        time_micros,
    )
}

mod coalesced_zoom_gestures_multiply;
mod every_sample_of_a_backed_up_stroke_is_kept;
mod input_merged_while_queued_keeps_its_order;
mod superseded_events_are_coalesced;
mod touch_moves_are_coalesced;
