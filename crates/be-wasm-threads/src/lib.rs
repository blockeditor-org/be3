// unsafe(ffi)
#[cfg(target_os = "wasi")]
unsafe extern "C" {
    fn __wasi_init_tp();
}

// unsafe(ffi)
pub fn initialize_main_thread() {
    #[cfg(target_os = "wasi")]
    unsafe {
        __wasi_init_tp();
    }
}

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
