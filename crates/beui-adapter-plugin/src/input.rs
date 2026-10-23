use std::time::Instant;

use beui_core::runner::Incoming;
use block_editor_plugin::{Region, TimedInput};

pub(crate) fn translate(region: &Region, event: &TimedInput) -> Incoming {
    Incoming::logical(event.input.clone().translated(region.rect.min.to_vec2()))
}

pub(crate) fn time(event: &TimedInput) -> Option<Instant> {
    event.time_micros.and_then(block_editor_plugin::instant_at)
}
