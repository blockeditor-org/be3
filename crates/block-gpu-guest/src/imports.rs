macro_rules! gpu_imports {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        #[cfg(target_arch = "wasm32")]
        #[link(wasm_import_module = "be3_gpu")]
        unsafe extern "C" {
            $(pub(crate) fn $name($($argument: $kind),*) $(-> $result)?;)+
        }

        #[cfg(not(target_arch = "wasm32"))]
        mod absent {
            $(
                pub(crate) unsafe fn $name($(_: $kind),*) $(-> $result)? {
                    unreachable!("the plugin gpu abi is only callable from wasm")
                }
            )+
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub(crate) use absent::*;
    };
}

block_gpu_abi::gpu_functions!(gpu_imports);
