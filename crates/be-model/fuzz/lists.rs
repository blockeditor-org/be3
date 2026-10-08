#![no_main]

#[unsafe(no_mangle)]
extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, size: usize) -> i32 {
    let data = match size {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(data, size) },
    };
    be_model::fuzz::lists(data);
    0
}
