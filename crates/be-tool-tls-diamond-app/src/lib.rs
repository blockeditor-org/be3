use be_tool_tls_diamond_base::bump_base;
use be_tool_tls_diamond_left::{bump_base_from_left, bump_left};
use be_tool_tls_diamond_right::{bump_base_from_right, bump_right};

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

// unsafe(ffi)
#[link(wasm_import_module = "be3_host")]
unsafe extern "C" {
    fn host_send(pointer: *const u8, length: usize);
    fn host_wake();
}

// A guest is single-threaded (decisions/guests_are_single_threaded.md), so the
// sequence runs on its one thread; it asks for a step, which checks that what
// it left stayed put between calls.
// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn app_start() {
    assert_eq!(sequence(), EXPECTED, "at the start");
    // unsafe(ffi)
    unsafe { host_wake() };
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn app_step() {
    let after = [bump_left(Step(0)), bump_right(Step(0)), bump_base(Step(0))];
    assert_eq!(after, [1001, 2002, 112], "on the step after the start");
    // unsafe(ffi)
    unsafe { host_send([1].as_ptr(), 1) };
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn app_shutdown() {}
