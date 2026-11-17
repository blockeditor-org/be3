use be_wasm_guest_abi::{Flow, Guest, Waker};

pub const STARTED: &[u8] = b"started";
pub const WOKEN: &[u8] = b"woken";
pub const PANIC: &[u8] = b"panic";
pub const HANG: &[u8] = b"hang";
pub const WAKE: &[u8] = b"wake";
pub const GROW: &[u8] = b"grow";
pub const FINISH: &[u8] = b"finish";
/// Asks for the wall clock through WASI, which only the staged app build (:app) has: answered with
/// `CLOCK_READ` once the clock reads after 2020.
pub const CLOCK: &[u8] = b"clock";
pub const CLOCK_READ: &[u8] = b"the clock reads after 2020";

/// The id the staged app build (:app) has in its manifest.
pub const APP_ID: &str = "be3.echo";

pub const PANIC_MESSAGE: &str = "the echo guest was asked to panic";

const GROWTH: usize = 1 << 30;

pub struct Echo {
    waker: Waker,
}

impl Guest for Echo {
    fn start(waker: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        outbound.push(STARTED.to_vec());
        Ok(Self { waker })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        if woken {
            outbound.push(WOKEN.to_vec());
        }
        for frame in inbound {
            match frame.as_slice() {
                PANIC => panic!("{PANIC_MESSAGE}"),
                HANG => hang(),
                WAKE => self.waker.wake(),
                GROW => {
                    let grown = std::hint::black_box(vec![1u8; GROWTH]);
                    outbound.push(vec![grown[GROWTH - 1]]);
                }
                FINISH => return Ok(Flow::Finished),
                CLOCK => {
                    let since = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|error| error.to_string())?;
                    if since.as_secs() > 1_577_836_800 {
                        outbound.push(CLOCK_READ.to_vec());
                    }
                }
                _ => outbound.push(frame),
            }
        }
        Ok(Flow::Running)
    }
}

fn hang() -> ! {
    let mut turns = 0u64;
    loop {
        turns = std::hint::black_box(turns.wrapping_add(1));
    }
}

be_wasm_guest_abi::guest!(Echo);
