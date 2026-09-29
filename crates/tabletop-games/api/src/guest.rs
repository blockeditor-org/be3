use std::convert::Infallible;

use crate::{Command, GameHelper, GameScreen};

const FIRST_BUFFER: usize = 256;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "game")]
unsafe extern "C" {
    #[link_name = "next"]
    fn host_next(buffer: u32, capacity: u32) -> u64;
    #[link_name = "present"]
    fn host_present(pointer: u32, length: u32);
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn host_next(_: u32, _: u32) -> u64 {
    panic!("only a game module running in the game host is handed commands")
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn host_present(_: u32, _: u32) {
    panic!("only a game module running in the game host presents screens")
}

pub fn text(value: &str) -> u64 {
    let bytes = value.as_bytes().to_vec().into_boxed_slice();
    let length = bytes.len() as u64;
    let pointer = Box::into_raw(bytes) as *mut u8 as u64;
    (pointer << 32) | length
}

pub(crate) fn next() -> Command {
    let mut buffer: Vec<u8> = Vec::with_capacity(FIRST_BUFFER);
    loop {
        let length =
            unsafe { host_next(buffer.as_mut_ptr() as u32, buffer.capacity() as u32) } as usize;
        if length <= buffer.capacity() {
            unsafe { buffer.set_len(length) };
            return bincode::deserialize(&buffer).expect("the host encodes the commands it sends");
        }
        buffer.reserve(length);
    }
}

pub(crate) fn present(screen: &GameScreen) {
    let bytes = bincode::serialize(screen).expect("a screen is always encodable");
    unsafe { host_present(bytes.as_ptr() as u32, bytes.len() as u32) };
}

pub fn play(game: fn(GameHelper<'_>) -> Result<Infallible, GameScreen>) {
    let screen = match game(GameHelper::hosted()) {
        Ok(never) => match never {},
        Err(screen) => screen,
    };
    loop {
        if let Command::Show(_) = next() {
            present(&screen);
        }
    }
}

#[macro_export]
macro_rules! game {
    ($name:expr, $play:path) => {
        #[unsafe(export_name = "name")]
        pub extern "C" fn game_module_name() -> u64 {
            $crate::guest::text($name)
        }

        #[unsafe(export_name = "play")]
        pub extern "C" fn game_module_play() {
            $crate::guest::play($play)
        }
    };
}
