#![no_main]

#[unsafe(no_mangle)]
extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, size: usize) -> i32 {
    let data = match size {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(data, size) },
    };
    beui_demo::fuzz::input(data);
    0
}
