#[cfg(target_arch = "wasm32")]
mod exports {
    use std::cell::RefCell;

    use be_remap::Module;

    thread_local! {
        static MODULE: RefCell<Module> = RefCell::new(Module::default());
        static REQUEST: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    #[unsafe(export_name = "be_remap_buffer")]
    pub extern "C" fn buffer(length: u32) -> u32 {
        REQUEST.with_borrow_mut(|request| {
            request.clear();
            request.resize(length as usize, 0);
            request.as_mut_ptr() as u32
        })
    }

    #[unsafe(export_name = "be_remap_handle")]
    pub extern "C" fn handle(length: u32) -> u64 {
        let reply = REQUEST.with_borrow(|request| {
            let length = (length as usize).min(request.len());
            MODULE.with_borrow_mut(|module| module.handle(&request[..length]))
        });
        REPLY.with_borrow_mut(|stored| {
            *stored = reply;
            (u64::from(stored.as_ptr() as u32) << 32) | stored.len() as u64
        })
    }
}
