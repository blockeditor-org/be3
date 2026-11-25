//! The WASI a pure guest is answered (guides/app_libraries.md, Pure guests): the few
//! `wasi_snapshot_preview1` calls Rust's std on `wasm32-wasip1` imports, answered so that a guest
//! stays deterministic. Its randomness is a ChaCha20 stream from the seed its host gives it, what
//! it writes to standard output or error goes to its host's log, its environment and arguments
//! are empty, it has no clock, and exiting stops it. Every other WASI call is refused when the
//! guest is loaded, as any import its host does not answer is.
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

/// One guest's WASI: its randomness, drawn in order from its seed.
pub struct Wasi {
    random: Keystream,
}

impl Wasi {
    pub fn new(seed: [u8; 32]) -> Self {
        Self {
            random: Keystream::new(seed),
        }
    }

    /// Answers the call `name` (one of `ANSWERED`) with its arguments as raw words: the errno it
    /// returns, or why the guest stops. What it writes to standard output or error is handed to
    /// `log`.
    pub fn call(
        &mut self,
        name: &str,
        arguments: &[u64],
        memory: &mut dyn Memory,
        log: &mut dyn FnMut(&[u8]),
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
            "fd_write" => fd_write(word(0), word(1), word(2), word(3), memory, log),
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
    log: &mut dyn FnMut(&[u8]),
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
        // A guest may name one buffer many times: what one call takes stays within its memory.
        if bytes.len() >= memory.size() {
            break;
        }
    }
    log(&bytes);
    memory.write(written, &(bytes.len() as u32).to_le_bytes())?;
    Ok(SUCCESS)
}

#[cfg(test)]
mod tests;
