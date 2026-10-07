use std::cell::Cell;
use std::time::{Duration, Instant};

thread_local! {
    static FRAME: Cell<Option<(Instant, Duration)>> = const { Cell::new(None) };
}

pub(crate) fn set_frame_time(now: Duration) {
    FRAME.with(|frame| {
        let epoch = frame.get().map_or_else(
            || {
                let real = Instant::now();
                real.checked_sub(now).unwrap_or(real)
            },
            |(epoch, _)| epoch,
        );
        frame.set(Some((epoch, now)));
    });
}

pub fn frame_time() -> Option<Instant> {
    FRAME.with(|frame| frame.get().map(|(epoch, now)| epoch + now))
}
