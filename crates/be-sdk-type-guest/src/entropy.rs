use std::cell::RefCell;

use rand_chacha::{
    ChaCha12Rng,
    rand_core::{RngCore, SeedableRng},
};

thread_local! {
    static GENERATOR: RefCell<Option<ChaCha12Rng>> = const { RefCell::new(None) };
}

pub(crate) fn seed(seed: [u8; 32]) {
    GENERATOR.with(|held| *held.borrow_mut() = Some(ChaCha12Rng::from_seed(seed)));
}

// unsafe(ffi)
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom::Error> {
    GENERATOR.with(|held| match held.borrow_mut().as_mut() {
        Some(generator) => {
            // unsafe(ffi)
            let dest = unsafe { std::slice::from_raw_parts_mut(dest, len) };
            generator.fill_bytes(dest);
            Ok(())
        }
        None => Err(getrandom::Error::UNSUPPORTED),
    })
}
