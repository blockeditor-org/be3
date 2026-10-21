use block_gpu_abi as abi;

macro_rules! transport_imports {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        #[link(wasm_import_module = "be3_host")]
        unsafe extern "C" {
            $(fn $name($($argument: $kind),*) $(-> $result)?;)+
        }
    };
}

block_gpu_abi::transport_functions!(transport_imports);

// unsafe(ffi)
#[link(wasm_import_module = "be3_host")]
unsafe extern "C" {
    fn host_wake();
    fn host_panic(pointer: u32, length: u32);
}

// unsafe(ffi)
pub(crate) fn report_panics() {
    std::panic::set_hook(Box::new(|info| {
        let message = info.payload_as_str().unwrap_or("a panic without a message");
        let report = match info.location() {
            Some(at) => format!(
                "{message}\n(at {}:{}:{})",
                at.file(),
                at.line(),
                at.column()
            ),
            None => message.to_owned(),
        };
        unsafe { host_panic(report.as_ptr() as u32, report.len() as u32) };
    }));
}

// unsafe(ffi)
pub(crate) fn send(frame: &[u8]) {
    unsafe { host_send(frame.as_ptr() as u32, frame.len() as u32) };
}

// unsafe(ffi)
pub(crate) fn receive() -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; 4096];
    let needed = unsafe { host_receive(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    if needed == abi::NO_MESSAGE {
        return None;
    }
    let needed = needed as usize;
    if needed > buffer.len() {
        buffer = vec![0u8; needed];
        let again = unsafe { host_receive(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
        if again == abi::NO_MESSAGE {
            return None;
        }
    }
    buffer.truncate(needed);
    Some(buffer)
}

// unsafe(ffi)
pub(crate) fn wake() {
    unsafe { host_wake() };
}
