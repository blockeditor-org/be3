use std::cell::Cell;

thread_local! {
    pub static LEFT: Cell<u32> = const { Cell::new(1000) };
}

pub fn bump_left<T: Into<u32>>(by: T) -> u32 {
    LEFT.with(|cell| {
        cell.set(cell.get() + by.into());
        cell.get()
    })
}

#[inline(never)]
pub fn bump_base_from_left(by: u32) -> u32 {
    tls_diamond_base::bump_base(by)
}
