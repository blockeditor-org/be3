use block_gpu_host::transport;
use wasmtime::{Caller, Linker};

use crate::State;

type Returns<T = ()> = T;

macro_rules! link_gpu {
    ($($glue:path, $module:literal, $name:literal, $export:literal, ($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)*) => {
        fn link_gpu(linker: &mut Linker<State>) -> Result<(), String> {
            $(
                linker
                    .func_wrap(
                        $module,
                        $name,
                        |mut caller: Caller<'_, State>, $($argument: $kind),*| -> wasmtime::Result<Returns<$($result)?>> {
                            let state = caller.data_mut();
                            let result = $glue(state, $($argument),*);
                            match state.fatal.is_some() {
                                false => Ok(result),
                                true => Err(crate::panic::stopped(state.fatal.take().unwrap_or_default())),
                            }
                        },
                    )
                    .map_err(|error| format!("{} could not be linked: {error}", $export))?;
            )*
            Ok(())
        }
    };
}

macro_rules! link_transport {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        fn link_transport(linker: &mut Linker<State>) -> Result<(), String> {
            $(
                linker
                    .func_wrap(
                        block_guest_abi::HOST_MODULE,
                        stringify!($name),
                        |mut caller: Caller<'_, State>, $($argument: $kind),*| $(-> $result)? {
                            transport::$name(caller.data_mut(), $($argument),*)
                        },
                    )
                    .map_err(|error| format!("{} could not be linked: {error}", stringify!($name)))?;
            )+
            Ok(())
        }
    };
}

block_gpu_abi::gpu_local_imports!(link_gpu);
block_guest_abi::transport_functions!(link_transport);

pub(super) fn link(linker: &mut Linker<State>) -> Result<(), String> {
    link_gpu(linker)?;
    link_transport(linker)
}
