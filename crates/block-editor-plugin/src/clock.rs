use std::cell::Cell;

thread_local! {
    static UTC_OFFSET: Cell<i32> = const { Cell::new(0) };
}

pub(crate) fn receive(seconds: i32) {
    UTC_OFFSET.with(|held| held.set(seconds));
}

pub fn utc_offset() -> i32 {
    UTC_OFFSET.with(Cell::get)
}
