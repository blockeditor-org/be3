use std::cell::Cell;

thread_local! {
    pub static RIGHT: Cell<u32> = const { Cell::new(2000) };
}

pub fn bump_right<T: Into<u32>>(by: T) -> u32 {
    RIGHT.with(|cell| {
        cell.set(cell.get() + by.into());
        cell.get()
    })
}

#[inline(never)]
pub fn bump_base_from_right(by: u32) -> u32 {
    tls_diamond_base::bump_base(by)
}
