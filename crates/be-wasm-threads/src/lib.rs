//! What a wasm32-wasip1-threads module needs from its host's threads.
//!
//! A browser traps `memory.atomic.wait` on a page's (a window's) thread, so std's `Mutex::lock`
//! traps there if it ever finds the lock held. The page never does: it shares its memory with no
//! other thread (guides/running/web.md, "Files and offline"), so every lock it takes is free.
//! Every other thread waits for real: a worker (`Atomics.wait`), a WASI thread under wasmtime,
//! and every native thread.

use std::cell::Cell;

// unsafe(ffi)
#[cfg(target_os = "wasi")]
unsafe extern "C" {
    fn __wasi_init_tp();
}

thread_local! {
    /// Set by `initialize_page`: this thread is a browser page's, which may not wait.
    static PAGE: Cell<bool> = const { Cell::new(false) };
}

fn initialize_thread_pointer() {
    // unsafe(ffi)
    #[cfg(target_os = "wasi")]
    unsafe {
        __wasi_init_tp();
    }
}

/// Starts a module instance on a browser page's own thread, which may never wait on a lock.
/// Every entry the page calls first may call it: after the first it does nothing.
pub fn initialize_page() {
    if PAGE.replace(true) {
        return;
    }
    initialize_thread_pointer();
}

/// Starts a module instance on a worker, which waits for a held lock as a native thread does.
pub fn initialize_worker() {
    initialize_thread_pointer();
}

/// Whether this thread may wait for a lock another thread holds: every thread but a browser
/// page's.
pub fn may_wait() -> bool {
    !PAGE.get()
}

#[cfg(test)]
mod tests;
