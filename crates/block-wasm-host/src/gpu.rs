use block_gpu_abi as abi;
use block_gpu_host::calls;
use wasmtime::{Caller, Linker};

use crate::State;

macro_rules! linker {
    ($link:ident, $module:expr; $(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        fn $link(linker: &mut Linker<State>) -> Result<(), String> {
            $(
                linker
                    .func_wrap(
                        $module,
                        stringify!($name),
                        |mut caller: Caller<'_, State>, $($argument: $kind),*| $(-> $result)? {
                            calls::$name(caller.data_mut(), $($argument),*)
                        },
                    )
                    .map_err(|error| format!("{} could not be linked: {error}", stringify!($name)))?;
            )+
            Ok(())
        }
    };
}

macro_rules! gpu {
    ($($functions:tt)+) => {
        linker!(link_gpu, abi::GPU_MODULE; $($functions)+);
    };
}

macro_rules! transport {
    ($($functions:tt)+) => {
        linker!(link_transport, block_guest_abi::HOST_MODULE; $($functions)+);
    };
}

abi::gpu_functions!(gpu);
block_guest_abi::transport_functions!(transport);

pub(super) fn link(linker: &mut Linker<State>) -> Result<(), String> {
    link_gpu(linker)?;
    link_transport(linker)
}
