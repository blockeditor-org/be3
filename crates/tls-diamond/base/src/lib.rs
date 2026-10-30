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

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn plugin_initialize_tls(size: u32, align: u32) {
    wasi_threads::initialize_main_thread_storage(size as usize, align as usize);
}
