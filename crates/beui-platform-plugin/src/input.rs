use std::time::Instant;

use beui_core::runner::Incoming;
use block_editor_plugin::TimedInput;

pub(crate) fn translate(event: &TimedInput) -> Incoming {
    Incoming::logical(event.input.clone())
}

pub(crate) fn time(event: &TimedInput) -> Option<Instant> {
    event.time_micros.and_then(block_editor_plugin::instant_at)
}
