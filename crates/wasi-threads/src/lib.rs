// unsafe(ffi)
#[cfg(target_os = "wasi")]
unsafe extern "C" {
    fn __wasi_init_tp();
    fn __wasm_init_tls(block: *mut u8);
}

// unsafe(ffi)
pub fn initialize_main_thread() {
    #[cfg(target_os = "wasi")]
    unsafe {
        __wasi_init_tp();
    }
}

// unsafe(ffi)
pub fn initialize_main_thread_storage(size: usize, align: usize) {
    #[cfg(target_os = "wasi")]
    unsafe {
        if size > 0
            && let Ok(layout) = std::alloc::Layout::from_size_align(size, align.max(1))
        {
            let block = std::alloc::alloc_zeroed(layout);
            if !block.is_null() {
                __wasm_init_tls(block);
            }
        }
    }
    initialize_main_thread();
    #[cfg(not(target_os = "wasi"))]
    {
        let _ = (size, align);
    }
}

// std's Mutex, except that on wasm a contended lock spins instead of waiting.
// A browser does not let the page's thread wait (memory.atomic.wait traps
// there), so a lock the page shares with a worker thread, such as the app's
// block stack, has to be this one. Elsewhere it is std's, unchanged.
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
        #[cfg(target_arch = "wasm32")]
        loop {
            match self.0.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => return Err(poisoned),
                Err(std::sync::TryLockError::WouldBlock) => std::hint::spin_loop(),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.0.lock()
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
