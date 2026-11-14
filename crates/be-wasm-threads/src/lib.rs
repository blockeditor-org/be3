//! What a wasm32-wasip1-threads module needs from its host's threads, and the one lock it may
//! share with a browser page's thread.
//!
//! A browser traps `memory.atomic.wait` on a page's (a window's) thread, so a lock that thread
//! finds held cannot be waited for there: std's `Mutex::lock` would trap once its brief spin
//! ends. Every other thread waits for real: a worker (`Atomics.wait`), a WASI thread under
//! wasmtime, and every native thread. guides/running/web.md says which locks the page shares.

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

/// A `std::sync::Mutex` for the locks a browser page shares with its workers. Off the page,
/// `lock` is std's, which waits for real. On the page it is the one place that still spins on a
/// held lock, until the page shares no lock with a thread that runs beside it
/// (plans/web_page_never_waits.md); a lock no other thread can hold never spins.
#[derive(Default)]
pub struct Mutex<T: ?Sized>(std::sync::Mutex<T>);

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self(std::sync::Mutex::new(value))
    }

    pub fn into_inner(self) -> std::sync::LockResult<T> {
        self.0.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    pub fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, T>> {
        if may_wait() {
            return self.0.lock();
        }
        loop {
            match self.0.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => return Err(poisoned),
                Err(std::sync::TryLockError::WouldBlock) => std::hint::spin_loop(),
            }
        }
    }

    pub fn try_lock(&self) -> std::sync::TryLockResult<std::sync::MutexGuard<'_, T>> {
        self.0.try_lock()
    }

    pub fn get_mut(&mut self) -> std::sync::LockResult<&mut T> {
        self.0.get_mut()
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for Mutex<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests;
