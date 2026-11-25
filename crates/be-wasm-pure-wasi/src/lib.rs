//! The WASI a pure guest is answered (guides/app_libraries.md, Pure guests): the few
//! `wasi_snapshot_preview1` calls Rust's std on `wasm32-wasip1` imports, answered so that a guest
//! stays deterministic. Its randomness is a ChaCha20 stream from the seed its host gives it, what
//! it writes to standard output or error goes to its host's log, its environment and arguments
//! are empty, it has no clock, and exiting stops it. Every other WASI call is refused when the
//! guest is loaded, as any import its host does not answer is.
//!
//! What it writes reaches the log a line at a time (`Wasi::call`'s `log`), which each host prefixes
//! with the guest's name.
//!
//! Every host of a pure guest answers these through `Wasi`: be-wasm-host's on V8, tabletop-host's
//! on wasmi and be-wasm-root-host's on the page. be-wasm-guest-host's worker (src/guest.js)
//! answers the same in JavaScript, with the same stream.

pub const MODULE: &str = "wasi_snapshot_preview1";

/// Every call a pure guest is answered, by name.
pub const ANSWERED: [&str; 9] = [
    "args_get",
    "args_sizes_get",
    "clock_res_get",
    "clock_time_get",
    "environ_get",
    "environ_sizes_get",
    "fd_write",
    "proc_exit",
    "random_get",
];

pub const SUCCESS: i32 = 0;
pub const EBADF: i32 = 8;
pub const ENOSYS: i32 = 52;

const STDOUT: u32 = 1;
const STDERR: u32 = 2;

/// A WebAssembly value type a call takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    I32,
    I64,
}

/// The parameters of the call `name`, one of `ANSWERED`, which returns an i32 errno unless it is
/// `proc_exit`, which returns nothing; None for any other name.
pub fn parameters(name: &str) -> Option<&'static [Word]> {
    use Word::{I32, I64};
    match name {
        "clock_time_get" => Some(&[I32, I64, I32]),
        "fd_write" => Some(&[I32, I32, I32, I32]),
        "proc_exit" => Some(&[I32]),
        "args_get" | "args_sizes_get" | "clock_res_get" | "environ_get" | "environ_sizes_get"
        | "random_get" => Some(&[I32, I32]),
        _ => None,
    }
}

/// Whether the call `name` returns an errno: every answered call but `proc_exit`.
pub fn returns_errno(name: &str) -> bool {
    name != "proc_exit"
}

/// Whether a pure guest's import of `module`.`name` is answered.
pub fn answers(module: &str, name: &str) -> bool {
    module == MODULE && ANSWERED.contains(&name)
}

/// The guest's memory, as a call reads and writes it.
pub trait Memory {
    /// Its size in bytes.
    fn size(&self) -> usize;
    /// `length` bytes at `at`, or why they are not all inside it.
    fn read(&self, at: u32, length: u32) -> Result<Vec<u8>, String>;
    /// Writes `bytes` at `at`, or says why they do not all fit inside it.
    fn write(&mut self, at: u32, bytes: &[u8]) -> Result<(), String>;
}

/// Why a call stopped its guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    /// It called `proc_exit` with this status.
    Exited(i32),
    /// It named memory it does not have.
    Fault(String),
}

impl std::fmt::Display for Stopped {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exited(status) => write!(formatter, "{EXITED}{status}"),
            Self::Fault(why) => formatter.write_str(why),
        }
    }
}

/// How a guest that called `proc_exit` is said to have stopped, followed by its status.
pub const EXITED: &str = "the guest exited with status ";

impl From<String> for Stopped {
    fn from(why: String) -> Self {
        Self::Fault(why)
    }
}

/// One guest's WASI: its randomness, drawn in order from its seed, and what it wrote since its
/// last whole line.
pub struct Wasi {
    random: Keystream,
    line: Vec<u8>,
}

impl Wasi {
    pub fn new(seed: [u8; 32]) -> Self {
        Self {
            random: Keystream::new(seed),
            line: Vec::new(),
        }
    }

    /// Answers the call `name` (one of `ANSWERED`) with its arguments as raw words: the errno it
    /// returns, or why the guest stops. Each whole line it has written to standard output or
    /// error is handed to `log` without its newline, and what is left of one when it exits.
    pub fn call(
        &mut self,
        name: &str,
        arguments: &[u64],
        memory: &mut dyn Memory,
        log: &mut dyn FnMut(&str),
    ) -> Result<i32, Stopped> {
        let answered = self.answer(name, arguments, memory);
        while let Some(end) = self.line.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.line.drain(..=end).collect();
            log(&String::from_utf8_lossy(&line[..end]));
        }
        if answered.is_err() && !self.line.is_empty() {
            log(&String::from_utf8_lossy(&std::mem::take(&mut self.line)));
        }
        answered
    }

    fn answer(
        &mut self,
        name: &str,
        arguments: &[u64],
        memory: &mut dyn Memory,
    ) -> Result<i32, Stopped> {
        let word = |index: usize| arguments.get(index).copied().unwrap_or(0) as u32;
        match name {
            // No arguments and no environment: none, in no bytes.
            "args_sizes_get" | "environ_sizes_get" => {
                memory.write(word(0), &0u32.to_le_bytes())?;
                memory.write(word(1), &0u32.to_le_bytes())?;
                Ok(SUCCESS)
            }
            "args_get" | "environ_get" => Ok(SUCCESS),
            "clock_res_get" | "clock_time_get" => Ok(ENOSYS),
            "fd_write" => fd_write(word(0), word(1), word(2), word(3), memory, &mut self.line),
            "proc_exit" => Err(Stopped::Exited(word(0) as i32)),
            "random_get" => {
                let (at, length) = (word(0), word(1));
                if (at as usize).saturating_add(length as usize) > memory.size() {
                    return Err(Stopped::Fault(format!(
                        "the guest asked for {length} random bytes at {at}, outside its memory"
                    )));
                }
                let mut bytes = vec![0; length as usize];
                self.random.fill(&mut bytes);
                memory.write(at, &bytes)?;
                Ok(SUCCESS)
            }
            _ => Ok(ENOSYS),
        }
    }
}

/// ChaCha20's keystream (RFC 7539) under the seed as its key, with a zero nonce and the block
/// counter from zero, drawn byte by byte: one draw goes on where the last stopped.
struct Keystream {
    key: [u32; 8],
    counter: u32,
    block: [u8; 64],
    used: usize,
}

impl Keystream {
    fn new(seed: [u8; 32]) -> Self {
        let mut key = [0; 8];
        for (word, bytes) in key.iter_mut().zip(seed.chunks_exact(4)) {
            *word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        Self {
            key,
            counter: 0,
            block: [0; 64],
            used: 64,
        }
    }

    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            if self.used == 64 {
                self.block = block(&self.key, self.counter);
                self.counter = self.counter.wrapping_add(1);
                self.used = 0;
            }
            *byte = self.block[self.used];
            self.used += 1;
        }
    }
}

fn quarter(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(7);
}

fn block(key: &[u32; 8], counter: u32) -> [u8; 64] {
    let mut initial = [0u32; 16];
    initial[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
    initial[4..12].copy_from_slice(key);
    initial[12] = counter;
    let mut state = initial;
    for _ in 0..10 {
        quarter(&mut state, 0, 4, 8, 12);
        quarter(&mut state, 1, 5, 9, 13);
        quarter(&mut state, 2, 6, 10, 14);
        quarter(&mut state, 3, 7, 11, 15);
        quarter(&mut state, 0, 5, 10, 15);
        quarter(&mut state, 1, 6, 11, 12);
        quarter(&mut state, 2, 7, 8, 13);
        quarter(&mut state, 3, 4, 9, 14);
    }
    let mut bytes = [0; 64];
    for (index, word) in state.iter().enumerate() {
        let word = word.wrapping_add(initial[index]);
        bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn read_u32(memory: &dyn Memory, at: u32) -> Result<u32, String> {
    let bytes = memory.read(at, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn fd_write(
    fd: u32,
    iovecs: u32,
    count: u32,
    written: u32,
    memory: &mut dyn Memory,
    line: &mut Vec<u8>,
) -> Result<i32, Stopped> {
    if fd != STDOUT && fd != STDERR {
        return Ok(EBADF);
    }
    if (iovecs as usize).saturating_add((count as usize).saturating_mul(8)) > memory.size() {
        return Err(Stopped::Fault(
            "the guest wrote from buffers outside its memory".to_owned(),
        ));
    }
    let mut bytes = Vec::new();
    for index in 0..count {
        let entry = iovecs.wrapping_add(index.wrapping_mul(8));
        let (pointer, length) = (
            read_u32(memory, entry)?,
            read_u32(memory, entry.wrapping_add(4))?,
        );
        bytes.extend(memory.read(pointer, length)?);
        // A guest may name one buffer many times: one call writes no more than its memory holds,
        // and says it wrote only that much, as a short write may.
        if bytes.len() >= memory.size() {
            bytes.truncate(memory.size());
            break;
        }
    }
    line.extend_from_slice(&bytes);
    memory.write(written, &(bytes.len() as u32).to_le_bytes())?;
    Ok(SUCCESS)
}

/// ChaCha20's keystream test vectors (RFC 7539, appendix A.1), which every host's tests check
/// its stream against, with the seed and block each is: a guest seeded with `seed` draws
/// `block`'s bytes as its stream's bytes `64 * block` onwards.
pub mod vectors {
    /// Test vector 1: the zero key's block 0.
    pub const ZERO_BLOCK_0: &str = "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7\
                                    da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586";
    /// Test vector 2: the zero key's block 1.
    pub const ZERO_BLOCK_1: &str = "9f07e7be5551387a98ba977c732d080dcb0f29a048e3656912c6533e32ee7aed\
                                    29b721769ce64e43d57133b074d839d531ed1f28510afb45ace10a1f4b794d6f";
    /// Test vector 3: block 1 under the key whose last byte is 1 (`ONE`).
    pub const ONE_BLOCK_1: &str = "3aeb5224ecf849929b9d828db1ced4dd832025e8018b8160b82284f3c949aa5a\
                                   8eca00bbb4a73bdad192b5c42f73f2fd4e273644c8b36125a64addeb006c13a0";
    /// The key whose last byte is 1.
    pub const ONE: [u8; 32] = {
        let mut key = [0; 32];
        key[31] = 1;
        key
    };

    /// `bytes` as lowercase hex, as the vectors are written.
    pub fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

#[cfg(test)]
mod tests;
