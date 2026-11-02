#[derive(Clone)]
pub struct Password(String);

impl Password {
    pub fn new(typed: String) -> Self {
        Self(typed)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// unsafe(memory)
impl Drop for Password {
    fn drop(&mut self) {
        let bytes = unsafe { self.0.as_bytes_mut() };
        for byte in bytes.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Password(..)")
    }
}
