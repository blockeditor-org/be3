use std::cell::Cell;

thread_local! {
    pub static BASE: Cell<u32> = const { Cell::new(100) };
}

pub fn bump_base<T: Into<u32>>(by: T) -> u32 {
    BASE.with(|cell| {
        cell.set(cell.get() + by.into());
        cell.get()
    })
}
