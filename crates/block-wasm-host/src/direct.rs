use block_gpu_host::{block_gpu_abi::embed::device::Caller as _, transport::Transported};
use block_guest_abi::direct;
use wasmtime::{Caller, Instance, Linker, Store};

use crate::state::{Guest, State};

pub(crate) fn link(linker: &mut Linker<State>, imports: &[(String, String)]) -> Result<(), String> {
    for (module, name) in imports {
        let full = format!("{module}#{name}");
        linker
            .func_wrap(
                module,
                name,
                move |mut caller: Caller<'_, State>,
                      has_parent: i32,
                      parent: i64,
                      pointer: u32,
                      length: u32|
                      -> wasmtime::Result<()> {
                    let mut guest = Guest::calling(&mut caller);
                    let args = guest.read(pointer, length).map_err(wasmtime::Error::msg)?;
                    let parent = (has_parent != 0).then_some(parent as u64);
                    guest
                        .transport()
                        .outbox
                        .push(direct::call(&full, parent, false, &args));
                    Ok(())
                },
            )
            .map_err(|error| format!("{module}#{name} could not be linked: {error}"))?;
    }
    Ok(())
}

pub(crate) fn call(
    instance: &Instance,
    store: &mut Store<State>,
    call: &direct::Call<'_>,
) -> Result<Vec<u8>, String> {
    let name = call.name;
    let alloc = instance
        .get_typed_func::<u32, u32>(&mut *store, direct::ALLOC)
        .map_err(|error| format!("the plugin has no {}: {error}", direct::ALLOC))?;
    let export = instance
        .get_typed_func::<(i32, i64, u32, u32), i64>(&mut *store, name)
        .map_err(|error| format!("the plugin does not export {name}: {error}"))?;
    let failed = |error: wasmtime::Error| {
        crate::panic::message(&error).unwrap_or_else(|| format!("{name} trapped: {error:?}"))
    };
    let length = call.args.len() as u32;
    let pointer = alloc.call(&mut *store, length).map_err(failed)?;
    Guest::of(&mut *store)
        .write(pointer, call.args)
        .map_err(|error| format!("the plugin's argument buffer is outside its memory: {error}"))?;
    let packed = export
        .call(
            &mut *store,
            (
                call.parent.is_some() as i32,
                call.parent.unwrap_or_default() as i64,
                pointer,
                length,
            ),
        )
        .map_err(failed)?;
    let (pointer, length) = direct::unpack(packed);
    Guest::of(store).read(pointer, length)
}
