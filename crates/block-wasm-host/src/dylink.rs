use wasm_dylink::{
    APPLY_DATA_RELOCS, Declared, ENV, GOT_THREAD_LOCAL, Import, PAGE_SIZE, STACK_POINTER, Sdk,
    TABLE, TLS_BASE, TLS_SIZE,
};
use wasmtime::{
    Error, Extern, Func, Global, GlobalType, Instance, Linker, Module, Mutability, Ref,
    SharedMemory, Store, Table, TypedFunc, Val, ValType,
};

pub(crate) use wasm_dylink::Layout;

#[derive(Clone)]
pub(crate) enum Program {
    Whole(Module),
    Linked { sdk: Module, plugin: Module },
}

impl Program {
    pub(crate) fn first(&self) -> &Module {
        match self {
            Self::Whole(module) => module,
            Self::Linked { sdk, .. } => sdk,
        }
    }

    pub(crate) fn modules(&self) -> Vec<&Module> {
        match self {
            Self::Whole(module) => vec![module],
            Self::Linked { sdk, plugin } => vec![sdk, plugin],
        }
    }
}

pub(crate) fn sdk_identity(module: &Module) -> Option<Result<String, String>> {
    wasm_dylink::sdk_identity(module.exports().map(|export| export.name()))
}

fn exported_i32<T>(
    instance: &Instance,
    store: &mut Store<T>,
    name: &str,
    what: &str,
) -> Result<i32, String> {
    instance
        .get_global(&mut *store, name)
        .and_then(|global| global.get(&mut *store).i32())
        .ok_or_else(|| format!("the {what} does not export {name}"))
}

fn global<T>(store: &mut Store<T>, value: i32, mutability: Mutability) -> Result<Global, String> {
    Global::new(
        &mut *store,
        GlobalType::new(ValType::I32, mutability),
        Val::I32(value),
    )
    .map_err(|error| format!("a plugin global could not be made: {error}"))
}

fn table<T>(sdk: &Instance, store: &mut Store<T>) -> Result<Table, String> {
    sdk.get_table(&mut *store, TABLE)
        .ok_or_else(|| format!("the plugin SDK does not export {TABLE}"))
}

fn write(memory: &SharedMemory, at: u32, bytes: &[u8]) -> Result<(), String> {
    let data = memory.data();
    let end = (at as usize)
        .checked_add(bytes.len())
        .filter(|end| *end <= data.len())
        .ok_or("the plugin's memory ran past the end of the shared memory")?;
    for (cell, byte) in data[at as usize..end].iter().zip(bytes) {
        // unsafe(ffi)
        unsafe { *cell.get() = *byte };
    }
    Ok(())
}

fn exports<T>(sdk: &Instance, store: &mut Store<T>) -> Sdk {
    let mut exports = Sdk::default();
    let named: Vec<(String, Extern)> = sdk
        .exports(&mut *store)
        .map(|export| (export.name().to_owned(), export.into_extern()))
        .collect();
    for (name, export) in named {
        match export {
            Extern::Func(_) => {
                exports.functions.insert(name);
            }
            Extern::Global(global) => {
                if let Some(address) = global.get(&mut *store).i32() {
                    exports.addresses.insert(name, address);
                }
            }
            _ => {}
        }
    }
    exports
}

pub(crate) fn plan<T>(
    store: &mut Store<T>,
    sdk: &Instance,
    plugin: &Module,
    memory: &SharedMemory,
) -> Result<Layout, String> {
    let declared = Declared::of(plugin.exports().map(|export| export.name()))?;
    let placed = memory
        .grow(u64::from(declared.pages()))
        .map_err(|error| format!("the memory could not grow to hold the plugin: {error}"))?;
    let placed = u32::try_from(placed * u64::from(PAGE_SIZE))
        .map_err(|_| "the plugin lies past a 32-bit address space".to_owned())?;
    let size = table(sdk, store)?.size(&mut *store);
    wasm_dylink::plan(
        declared,
        placed,
        size,
        plugin
            .imports()
            .map(|import| (import.module(), import.name())),
        &exports(sdk, store),
    )
}

pub(crate) fn instantiate<T>(
    store: &mut Store<T>,
    linker: &Linker<T>,
    sdk: &Instance,
    plugin: &Module,
    layout: &Layout,
) -> Result<Instance, String> {
    let table = table(sdk, store)?;
    table
        .grow(&mut *store, layout.table_grow, Ref::Func(None))
        .map_err(|error| format!("the function table could not grow for the plugin: {error}"))?;
    for (slot, name) in &layout.sdk_slots {
        let function = sdk
            .get_func(&mut *store, name)
            .ok_or_else(|| format!("the plugin SDK does not export {name}"))?;
        table
            .set(&mut *store, *slot, Ref::Func(Some(function)))
            .map_err(|error| format!("{name} could not be put in the function table: {error}"))?;
    }
    let stack_pointer = sdk
        .get_global(&mut *store, STACK_POINTER)
        .ok_or_else(|| format!("the plugin SDK does not export {STACK_POINTER}"))?;
    let mut imports = Vec::new();
    let mut own = Vec::new();
    for (import, resolution) in plugin.imports().zip(&layout.imports) {
        let (module, name) = (import.module(), import.name());
        let resolved: Extern = match *resolution {
            Import::Table => table.into(),
            Import::StackPointer => stack_pointer.into(),
            Import::Constant(value) => global(store, value, Mutability::Const)?.into(),
            Import::Slot(value) | Import::Address(value) => {
                global(store, value, Mutability::Var)?.into()
            }
            Import::Own => {
                let entry = global(store, 0, Mutability::Var)?;
                own.push((name.to_owned(), entry));
                entry.into()
            }
            Import::Elsewhere => match linker.get_by_import(&mut *store, &import) {
                Some(answered) => answered,
                None if module == ENV => sdk
                    .get_export(&mut *store, name)
                    .filter(|export| matches!(export, Extern::Func(_) | Extern::Tag(_)))
                    .ok_or_else(|| {
                        format!("the plugin imports {name}, which the plugin SDK does not export")
                    })?,
                None => {
                    let signature = import.ty().func().cloned().ok_or_else(|| {
                        format!(
                            "the plugin imports {module}.{name}, which its host does not answer"
                        )
                    })?;
                    let refusal = format!(
                        "the plugin called {module}.{name}, which its host does not answer"
                    );
                    Func::new(&mut *store, signature, move |_, _, _| {
                        Err(Error::msg(refusal.clone()))
                    })
                    .into()
                }
            },
        };
        imports.push(resolved);
    }
    let instance = Instance::new(&mut *store, plugin, &imports)
        .map_err(|error| format!("the plugin could not be instantiated: {error}"))?;
    for (name, entry) in own {
        let offset = exported_i32(&instance, store, &name, "plugin")?;
        let address = layout.own_address(&name, offset)?;
        entry
            .set(&mut *store, Val::I32(address as i32))
            .map_err(|error| format!("{name} could not be relocated: {error}"))?;
    }
    for (slot, name) in &layout.plugin_slots {
        let function = instance.get_func(&mut *store, name).ok_or_else(|| {
            format!("the plugin takes the address of {name}, which nothing exports")
        })?;
        table
            .set(&mut *store, *slot, Ref::Func(Some(function)))
            .map_err(|error| format!("{name} could not be put in the function table: {error}"))?;
    }
    Ok(instance)
}

pub(crate) fn relocate<T>(
    store: &mut Store<T>,
    plugin: &Instance,
    layout: &mut Layout,
) -> Result<(), String> {
    layout.relocated(
        exported_i32(plugin, store, TLS_BASE, "plugin")? as u32,
        exported_i32(plugin, store, TLS_SIZE, "plugin")? as u32,
    );
    let relocate: TypedFunc<(), ()> = plugin
        .get_typed_func(&mut *store, APPLY_DATA_RELOCS)
        .map_err(|error| format!("the plugin has no usable {APPLY_DATA_RELOCS}: {error}"))?;
    relocate
        .call(&mut *store, ())
        .map_err(|error| format!("the plugin's data could not be relocated: {error}"))
}

pub(crate) fn place_thread_locals<T>(
    store: &mut Store<T>,
    plugin: &Instance,
    memory: &SharedMemory,
    layout: &Layout,
    base: u32,
) -> Result<(), String> {
    if let Some(image) = layout.thread_locals(base)? {
        let bytes = crate::state::read(memory, image.from, image.length)?;
        write(memory, image.to, &bytes)?;
    }
    plugin
        .get_global(&mut *store, TLS_BASE)
        .ok_or_else(|| format!("the plugin does not export {TLS_BASE}"))?
        .set(&mut *store, Val::I32(base as i32))
        .map_err(|error| format!("the plugin's thread-locals could not be placed: {error}"))
}

pub(crate) fn main_thread_locals<T>(store: &mut Store<T>, sdk: &Instance) -> Result<u32, String> {
    exported_i32(sdk, store, TLS_BASE, "plugin SDK").map(|base| base as u32)
}

pub(crate) fn spawned_thread_locals(
    memory: &SharedMemory,
    start_argument: i32,
) -> Result<u32, String> {
    let at = wasm_dylink::spawned_thread_locals(start_argument)?;
    let bytes = crate::state::read(memory, at, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn thread_local_imports<T>(
    store: &mut Store<T>,
    module: &Module,
) -> Result<Vec<(String, Global)>, String> {
    module
        .imports()
        .filter(|import| import.module() == GOT_THREAD_LOCAL)
        .map(|import| Ok((import.name().to_owned(), global(store, 0, Mutability::Var)?)))
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn place_other_thread_locals<T>(
    store: &mut Store<T>,
    globals: &[(String, Global)],
    own_base: u32,
    mut address: impl FnMut(&mut Store<T>, &str) -> Result<u32, String>,
) -> Result<(), String> {
    for (name, entry) in globals {
        let slot = wasm_dylink::thread_local_slot(address(store, name)?, own_base);
        entry
            .set(&mut *store, Val::I32(slot))
            .map_err(|error| format!("the thread-local {name} could not be placed: {error}"))?;
    }
    Ok(())
}
