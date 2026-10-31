use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use tls_diamond_base::bump_base;
use tls_diamond_left::{bump_base_from_left, bump_left};
use tls_diamond_right::{bump_base_from_right, bump_right};

struct Step(u32);

impl From<Step> for u32 {
    fn from(step: Step) -> u32 {
        step.0
    }
}

const EXPECTED: [u32; 6] = [1001, 2002, 103, 107, 112, 112];

fn sequence() -> [u32; 6] {
    [
        bump_left(Step(1)),
        bump_right(Step(2)),
        bump_base(Step(3)),
        bump_base_from_left(4),
        bump_base_from_right(5),
        bump_base(Step(0)),
    ]
}

static SPAWNED: Mutex<Option<JoinHandle<[u32; 6]>>> = Mutex::new(None);
static ENDED: AtomicBool = AtomicBool::new(false);

// unsafe(ffi)
#[link(wasm_import_module = "be3_host")]
unsafe extern "C" {
    fn host_send(pointer: *const u8, length: usize);
    fn host_wake();
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn plugin_start() {
    assert_eq!(sequence(), EXPECTED, "on the main thread");
    let spawned = std::thread::spawn(|| {
        let seen = sequence();
        ENDED.store(true, Ordering::Release);
        // unsafe(ffi)
        unsafe { host_wake() };
        seen
    });
    *SPAWNED.lock().unwrap() = Some(spawned);
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn plugin_step() {
    if !ENDED.load(Ordering::Acquire) {
        return;
    }
    let Some(spawned) = SPAWNED.lock().unwrap().take() else {
        return;
    };
    assert_eq!(spawned.join().unwrap(), EXPECTED, "on a spawned thread");
    let after = [bump_left(Step(0)), bump_right(Step(0)), bump_base(Step(0))];
    assert_eq!(
        after,
        [1001, 2002, 112],
        "on the main thread after the spawned one"
    );
    // unsafe(ffi)
    unsafe { host_send([1].as_ptr(), 1) };
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn plugin_shutdown() {}
