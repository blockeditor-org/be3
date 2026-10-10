use std::cell::Cell;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

thread_local! {
    static FRAME: Cell<Option<(Instant, Duration)>> = const { Cell::new(None) };
    static UTC_OFFSET: Cell<i32> = const { Cell::new(0) };
    static PINNED_WALL_CLOCK: Cell<Option<Duration>> = const { Cell::new(None) };
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

pub(crate) fn receive(seconds: i32) {
    UTC_OFFSET.with(|held| held.set(seconds));
}

pub fn utc_offset() -> i32 {
    UTC_OFFSET.with(Cell::get)
}

pub fn pin_wall_clock(since_epoch: Option<Duration>) {
    PINNED_WALL_CLOCK.with(|held| held.set(since_epoch));
}

pub fn wall_clock() -> Duration {
    match PINNED_WALL_CLOCK.with(Cell::get) {
        Some(pinned) => {
            pinned + FRAME.with(|frame| frame.get().map_or(Duration::ZERO, |(_, now)| now))
        }
        None => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default(),
    }
}
