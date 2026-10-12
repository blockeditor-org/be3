use wasmtime::{
    Error, Extern, Func, Global, GlobalType, Instance, Linker, Module, Mutability, Ref,
    SharedMemory, Store, Table, TypedFunc, Val, ValType,
};

const SDK_EXPORT_PREFIX: &str = "__be3_sdk_";
const LAYOUT_EXPORT_PREFIX: &str = "__be3_dylink_";
const SDK_FILE_PREFIX: &str = "sdk-";
const PLUGIN_TLS: &str = "__be3_plugin_tls";
const TABLE: &str = "__indirect_function_table";
const STACK_POINTER: &str = "__stack_pointer";
const MEMORY_BASE: &str = "__memory_base";
const TABLE_BASE: &str = "__table_base";
const TLS_BASE: &str = "__tls_base";
const TLS_SIZE: &str = "__tls_size";
const MALLOC: &str = "malloc";
const APPLY_DATA_RELOCS: &str = "__wasm_apply_data_relocs";
const ENV: &str = "env";
const GOT_MEMORY: &str = "GOT.mem";
const GOT_FUNCTION: &str = "GOT.func";
const START_ARGUMENT_TLS: u32 = 4;
const MAX_ALIGN_LOG2: u32 = 16;
const IDENTITY_LENGTH: usize = 64;

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

pub fn sdk_identity(module: &Module) -> Option<Result<String, String>> {
    module
        .exports()
        .find_map(|export| export.name().strip_prefix(SDK_EXPORT_PREFIX))
        .map(|identity| {
            let valid = identity.len() == IDENTITY_LENGTH
                && identity
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
            valid.then(|| identity.to_owned()).ok_or_else(|| {
                format!("the plugin names {identity:?} as its plugin SDK, which is not a SHA-256")
            })
        })
}

pub fn sdk_file_name(identity: &str) -> String {
    format!("{SDK_FILE_PREFIX}{identity}.wasm")
}

#[derive(Clone, Copy)]
struct Declared {
    memory_size: u32,
    memory_align: u32,
    table_size: u32,
    table_align: u32,
}

fn declared(plugin: &Module) -> Result<Declared, String> {
    let layout = plugin
        .exports()
        .find_map(|export| export.name().strip_prefix(LAYOUT_EXPORT_PREFIX))
        .ok_or("the plugin does not say how much memory and table it needs")?;
    let numbers: Vec<u32> = layout
        .split('_')
        .map(|number| number.parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("the plugin's layout {layout} is not one this reads"))?;
    let [memory_size, memory_align, table_size, table_align] = numbers[..] else {
        return Err(format!(
            "the plugin's layout {layout} is not one this reads"
        ));
    };
    Ok(Declared {
        memory_size,
        memory_align: memory_align.min(MAX_ALIGN_LOG2),
        table_size,
        table_align: table_align.min(MAX_ALIGN_LOG2),
    })
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    memory_base: u32,
    table_base: u32,
    table_grow: u64,
    sdk_slots: Vec<(u64, String)>,
    plugin_slots: Vec<(u64, String)>,
    data: Vec<(String, i32)>,
    reserve: u32,
    tls_size: u32,
    image: u32,
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

fn aligned(value: u32, align_log2: u32) -> Option<u32> {
    value.div_ceil(1 << align_log2).checked_mul(1 << align_log2)
}

fn write(memory: &SharedMemory, at: u32, bytes: &[u8]) -> Result<(), String> {
    let data = memory.data();
    let end = (at as usize)
        .checked_add(bytes.len())
        .filter(|end| *end <= data.len())
        .ok_or("the plugin's memory ran past the end of the shared memory")?;
    for (cell, byte) in data[at as usize..end].iter().zip(bytes) {
        unsafe { *cell.get() = *byte };
    }
    Ok(())
}

pub(crate) fn plan<T>(
    store: &mut Store<T>,
    sdk: &Instance,
    plugin: &Module,
    memory: &SharedMemory,
) -> Result<Layout, String> {
    let declared = declared(plugin)?;
    let malloc: TypedFunc<u32, u32> = sdk
        .get_typed_func(&mut *store, MALLOC)
        .map_err(|error| format!("the plugin SDK has no usable {MALLOC}: {error}"))?;
    let too_big = || "the plugin asks for more than a 32-bit address space holds".to_owned();
    let request = declared
        .memory_size
        .checked_add(1 << declared.memory_align)
        .ok_or_else(too_big)?;
    let allocated = malloc
        .call(&mut *store, request)
        .map_err(|error| format!("the plugin's memory could not be allocated: {error}"))?;
    if allocated == 0 {
        return Err("the plugin SDK could not allocate the plugin's memory".to_owned());
    }
    let memory_base = aligned(allocated, declared.memory_align).ok_or_else(too_big)?;
    write(memory, memory_base, &vec![0; declared.memory_size as usize])?;
    let size = table(sdk, store)?.size(&mut *store);
    let table_base = u32::try_from(size)
        .ok()
        .and_then(|size| aligned(size, declared.table_align))
        .ok_or_else(too_big)?;
    let mut next = u64::from(table_base) + u64::from(declared.table_size);
    let mut sdk_slots = Vec::new();
    let mut plugin_slots = Vec::new();
    let mut data = Vec::new();
    for import in plugin.imports() {
        match import.module() {
            GOT_FUNCTION => {
                let slots = match sdk.get_func(&mut *store, import.name()) {
                    Some(_) => &mut sdk_slots,
                    None => &mut plugin_slots,
                };
                slots.push((next, import.name().to_owned()));
                next += 1;
            }
            GOT_MEMORY => {
                if let Some(address) = sdk
                    .get_global(&mut *store, import.name())
                    .and_then(|global| global.get(&mut *store).i32())
                {
                    data.push((import.name().to_owned(), address));
                }
            }
            _ => {}
        }
    }
    Ok(Layout {
        memory_base,
        table_base,
        table_grow: next - size,
        sdk_slots,
        plugin_slots,
        data,
        reserve: exported_i32(sdk, store, PLUGIN_TLS, "plugin SDK")? as u32,
        tls_size: 0,
        image: 0,
    })
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
    for import in plugin.imports() {
        let (module, name) = (import.module(), import.name());
        let resolved: Extern = match (module, name) {
            (ENV, TABLE) => table.into(),
            (ENV, STACK_POINTER) => stack_pointer.into(),
            (ENV, MEMORY_BASE) => {
                global(store, layout.memory_base as i32, Mutability::Const)?.into()
            }
            (ENV, TABLE_BASE) => global(store, layout.table_base as i32, Mutability::Const)?.into(),
            (GOT_FUNCTION, _) => {
                let slot = layout
                    .sdk_slots
                    .iter()
                    .chain(&layout.plugin_slots)
                    .find(|(_, symbol)| symbol == name)
                    .map_or(0, |(slot, _)| *slot as i32);
                global(store, slot, Mutability::Var)?.into()
            }
            (GOT_MEMORY, _) => {
                let address = layout
                    .data
                    .iter()
                    .find(|(symbol, _)| symbol == name)
                    .map(|(_, address)| *address);
                let entry = global(store, address.unwrap_or(0), Mutability::Var)?;
                if address.is_none() {
                    own.push((name.to_owned(), entry));
                }
                entry.into()
            }
            _ => match linker.get_by_import(&mut *store, &import) {
                Some(answered) => answered,
                None if module == ENV => sdk
                    .get_func(&mut *store, name)
                    .ok_or_else(|| {
                        format!("the plugin imports {name}, which the plugin SDK does not export")
                    })?
                    .into(),
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
        let address = layout
            .memory_base
            .checked_add_signed(offset)
            .ok_or_else(|| format!("{name} lies past a 32-bit address space"))?;
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
    layout.image = exported_i32(plugin, store, TLS_BASE, "plugin")? as u32;
    layout.tls_size = exported_i32(plugin, store, TLS_SIZE, "plugin")? as u32;
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
    let start = layout.reserve;
    if layout.tls_size > start {
        let overflow = || "the plugin's thread-locals lie past a 32-bit address space".to_owned();
        let from = layout.image.checked_add(start).ok_or_else(overflow)?;
        let to = base.checked_add(start).ok_or_else(overflow)?;
        let image = crate::state::read(memory, from, layout.tls_size - start)?;
        write(memory, to, &image)?;
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
    let at = (start_argument as u32)
        .checked_add(START_ARGUMENT_TLS)
        .ok_or("a spawned thread's start argument lies past a 32-bit address space")?;
    let bytes = crate::state::read(memory, at, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
