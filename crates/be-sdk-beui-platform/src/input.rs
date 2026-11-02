use std::time::Instant;

use be_sdk::TimedInput;
use beui_runner::Incoming;

pub(crate) fn translate(event: &TimedInput) -> Incoming {
    Incoming::logical(event.input.clone())
}

pub(crate) fn time(event: &TimedInput) -> Option<Instant> {
    event.time_micros.and_then(be_sdk::instant_at)
}
