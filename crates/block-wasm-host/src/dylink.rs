use std::sync::{Arc, Mutex};

use wasm_dylink::{
    APPLY_DATA_RELOCS, CONSTRUCTORS, Described, INIT_TLS, Import, PAGE_SIZE, Plan, STACK_POINTER,
    TABLE, TLS_BASE,
};
use wasmtime::{
    Error, Extern, ExternType, Func, Global, GlobalType, Instance, Linker, Module, Mutability, Ref,
    SharedMemory, Store, Table, Val, ValType,
};

#[derive(Clone)]
pub(crate) enum Program {
    Whole(Module),
    Linked(Arc<[Module]>),
}

impl Program {
    pub(crate) fn first(&self) -> &Module {
        match self {
            Self::Whole(module) => module,
            Self::Linked(modules) => &modules[0],
        }
    }

    pub(crate) fn modules(&self) -> &[Module] {
        match self {
            Self::Whole(module) => std::slice::from_ref(module),
            Self::Linked(modules) => modules,
        }
    }
}

pub(crate) fn libraries(module: &Module) -> Result<Vec<String>, String> {
    wasm_dylink::libraries(module.exports().map(|export| export.name()))
}

pub(crate) struct Linked {
    pub(crate) plan: Plan,
    free: Mutex<Vec<u32>>,
}

impl Linked {
    pub(crate) fn take(&self, memory: &SharedMemory) -> Result<u32, String> {
        let blocks = self.plan.blocks;
        if blocks.size == 0 {
            return Ok(0);
        }
        let mut free = self
            .free
            .lock()
            .map_err(|_| "the thread-local blocks were poisoned".to_owned())?;
        if free.is_empty() {
            let pages = blocks.pages;
            let start = memory.grow(u64::from(pages)).map_err(|error| {
                format!("the memory could not grow for thread-local storage: {error}")
            })?;
            let start = u32::try_from(start * u64::from(PAGE_SIZE))
                .map_err(|_| "thread-local storage lies past a 32-bit address space".to_owned())?;
            free.extend(blocks.carve(start, pages).into_iter().rev());
        }
        let block = free
            .pop()
            .ok_or("no thread-local block fits in the pages grown for them")?;
        write(memory, block, &vec![0; blocks.size as usize])?;
        Ok(block)
    }

    pub(crate) fn give(&self, block: u32) {
        if self.plan.blocks.size == 0 {
            return;
        }
        if let Ok(mut free) = self.free.lock() {
            free.push(block);
        }
    }
}

fn describe(module: &Module, name: String) -> Described {
    let mut described = Described {
        name,
        imports: module
            .imports()
            .map(|import| (import.module().to_owned(), import.name().to_owned()))
            .collect(),
        ..Described::default()
    };
    for export in module.exports() {
        let name = export.name().to_owned();
        match export.ty() {
            ExternType::Func(_) | ExternType::Tag(_) => {
                described.functions.insert(name.clone());
            }
            ExternType::Global(_) => {
                described.globals.insert(name.clone());
            }
            _ => {}
        }
        described.exports.push(name);
    }
    described
}

fn exported_i32<T>(instance: &Instance, store: &mut Store<T>, name: &str) -> Option<i32> {
    instance
        .get_global(&mut *store, name)
        .and_then(|global| global.get(&mut *store).i32())
}

fn global<T>(store: &mut Store<T>, value: i32, mutability: Mutability) -> Result<Global, String> {
    Global::new(
        &mut *store,
        GlobalType::new(ValType::I32, mutability),
        Val::I32(value),
    )
    .map_err(|error| format!("a plugin global could not be made: {error}"))
}

fn table<T>(root: &Instance, store: &mut Store<T>) -> Result<Table, String> {
    root.get_table(&mut *store, TABLE)
        .ok_or_else(|| format!("the root library does not export {TABLE}"))
}

fn write(memory: &SharedMemory, at: u32, bytes: &[u8]) -> Result<(), String> {
    let data = memory.data();
    let end = (at as usize)
        .checked_add(bytes.len())
        .filter(|end| *end <= data.len())
        .ok_or("thread-local storage ran past the end of the shared memory")?;
    for (cell, byte) in data[at as usize..end].iter().zip(bytes) {
        // unsafe(ffi)
        unsafe { *cell.get() = *byte };
    }
    Ok(())
}

fn name(modules: &[Module], index: usize, identities: &[String]) -> String {
    match identities.get(index) {
        _ if index + 1 == modules.len() => "the plugin".to_owned(),
        Some(identity) => format!("the library {identity}"),
        None => format!("module {index}"),
    }
}

pub(crate) fn plan<T>(
    store: &mut Store<T>,
    modules: &[Module],
    identities: &[String],
    root: &Instance,
    memory: &SharedMemory,
) -> Result<Arc<Linked>, String> {
    let described: Vec<Described> = modules
        .iter()
        .enumerate()
        .map(|(index, module)| describe(module, name(modules, index, identities)))
        .collect();
    let pages = wasm_dylink::pages(&described)?;
    let placed = memory
        .grow(u64::from(pages))
        .map_err(|error| format!("the memory could not grow to hold the plugin: {error}"))?;
    let placed = u32::try_from(placed * u64::from(PAGE_SIZE))
        .map_err(|_| "the plugin lies past a 32-bit address space".to_owned())?;
    let size = table(root, store)?.size(&mut *store);
    Ok(Arc::new(Linked {
        plan: wasm_dylink::plan(&described, placed, size)?,
        free: Mutex::new(Vec::new()),
    }))
}

pub(crate) struct Thread {
    pub(crate) blocks: u32,
    pub(crate) root_block: u32,
    pub(crate) main: bool,
}

fn fill_slots<T>(
    store: &mut Store<T>,
    table: &Table,
    plan: &Plan,
    instance: &Instance,
    module: usize,
) -> Result<(), String> {
    for slot in plan.slots.iter().filter(|slot| slot.module == module) {
        let function = instance.get_func(&mut *store, &slot.name).ok_or_else(|| {
            format!(
                "the address of {} is taken, and nothing exports it",
                slot.name
            )
        })?;
        table
            .set(&mut *store, slot.slot, Ref::Func(Some(function)))
            .map_err(|error| {
                format!(
                    "{} could not be put in the function table: {error}",
                    slot.name
                )
            })?;
    }
    Ok(())
}

fn unexported(name: &str) -> String {
    format!("{name} is imported, and the module the plan names does not export it")
}

fn answered<T>(
    store: &mut Store<T>,
    linker: &Linker<T>,
    import: &wasmtime::ImportType<'_>,
) -> Result<Extern, String> {
    if let Some(answered) = linker.get_by_import(&mut *store, import) {
        return Ok(answered);
    }
    let (from, name) = (import.module(), import.name());
    let signature = import.ty().func().cloned().ok_or_else(|| {
        format!("the plugin imports {from}.{name}, which its host does not answer")
    })?;
    let refusal = format!("the plugin called {from}.{name}, which its host does not answer");
    Ok(Func::new(&mut *store, signature, move |_, _, _| {
        Err(Error::msg(refusal.clone()))
    })
    .into())
}

pub(crate) fn link<T>(
    store: &mut Store<T>,
    linker: &Linker<T>,
    modules: &[Module],
    linked: &Linked,
    root: Instance,
    thread: &Thread,
) -> Result<Instance, String> {
    let plan = &linked.plan;
    let table = table(&root, store)?;
    table
        .grow(&mut *store, plan.table_grow, Ref::Func(None))
        .map_err(|error| format!("the function table could not grow for the plugin: {error}"))?;
    fill_slots(store, &table, plan, &root, 0)?;
    let stack_pointer = root
        .get_global(&mut *store, STACK_POINTER)
        .ok_or_else(|| format!("the root library does not export {STACK_POINTER}"))?;
    let mut instances = vec![root];
    for (index, module) in modules.iter().enumerate().skip(1) {
        let placed = &plan.modules[index];
        let mut imports = Vec::new();
        let mut own = Vec::new();
        let mut thread_locals = Vec::new();
        for (import, resolution) in module.imports().zip(&placed.imports) {
            let name = import.name();
            let resolved: Extern = match *resolution {
                Import::Table => table.into(),
                Import::StackPointer => stack_pointer.into(),
                Import::Constant(value) => global(store, value, Mutability::Const)?.into(),
                Import::FunctionSlot(value) => global(store, value, Mutability::Var)?.into(),
                Import::Function(definer) => instances[definer]
                    .get_export(&mut *store, name)
                    .filter(|export| matches!(export, Extern::Func(_) | Extern::Tag(_)))
                    .ok_or_else(|| unexported(name))?,
                Import::Address(definer) if definer < index => {
                    let value = exported_i32(&instances[definer], store, name)
                        .ok_or_else(|| unexported(name))?;
                    global(store, plan.address(definer, value)? as i32, Mutability::Var)?.into()
                }
                Import::Address(_) => {
                    let entry = global(store, 0, Mutability::Var)?;
                    own.push((name.to_owned(), entry));
                    entry.into()
                }
                Import::ThreadLocal(definer) => {
                    let entry = global(store, 0, Mutability::Var)?;
                    thread_locals.push((name.to_owned(), definer, entry));
                    entry.into()
                }
                Import::Elsewhere => answered(store, linker, &import)?,
            };
            imports.push(resolved);
        }
        let instance = Instance::new(&mut *store, module, &imports)
            .map_err(|error| format!("the plugin could not be instantiated: {error}"))?;
        for (name, entry) in own {
            let value = exported_i32(&instance, store, &name).ok_or_else(|| unexported(&name))?;
            entry
                .set(&mut *store, Val::I32(plan.address(index, value)? as i32))
                .map_err(|error| format!("{name} could not be relocated: {error}"))?;
        }
        fill_slots(store, &table, plan, &instance, index)?;
        if thread.main
            && let Ok(relocate) = instance.get_typed_func::<(), ()>(&mut *store, APPLY_DATA_RELOCS)
        {
            relocate
                .call(&mut *store, ())
                .map_err(|error| format!("the plugin's data could not be relocated: {error}"))?;
        }
        if let Ok(initialize) = instance.get_typed_func::<u32, ()>(&mut *store, INIT_TLS) {
            initialize
                .call(&mut *store, plan.block(thread.blocks, index))
                .map_err(|error| format!("thread-local storage could not be set up: {error}"))?;
        }
        let own_base = exported_i32(&instance, store, TLS_BASE).unwrap_or(0) as u32;
        for (name, definer, entry) in thread_locals {
            let base = match definer {
                0 => thread.root_block,
                _ => exported_i32(&instances[definer], store, TLS_BASE).unwrap_or(0) as u32,
            };
            let offset =
                exported_i32(&instances[definer], store, &name).ok_or_else(|| unexported(&name))?;
            let slot = wasm_dylink::thread_local_slot(base.wrapping_add_signed(offset), own_base);
            entry
                .set(&mut *store, Val::I32(slot))
                .map_err(|error| format!("the thread-local {name} could not be placed: {error}"))?;
        }
        if thread.main
            && let Ok(constructors) = instance.get_typed_func::<(), ()>(&mut *store, CONSTRUCTORS)
        {
            constructors
                .call(&mut *store, ())
                .map_err(|error| format!("the plugin could not run its constructors: {error}"))?;
        }
        instances.push(instance);
    }
    instances
        .pop()
        .ok_or_else(|| "there is no module to link".to_owned())
}

pub(crate) fn main_thread_locals<T>(store: &mut Store<T>, root: &Instance) -> Result<u32, String> {
    exported_i32(root, store, TLS_BASE)
        .map(|base| base as u32)
        .ok_or_else(|| format!("the root library does not export {TLS_BASE}"))
}

pub(crate) fn spawned_thread_locals(
    memory: &SharedMemory,
    start_argument: i32,
) -> Result<u32, String> {
    let at = wasm_dylink::spawned_thread_locals(start_argument)?;
    let bytes = crate::state::read(memory, at, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
