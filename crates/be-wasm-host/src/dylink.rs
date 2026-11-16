//! Carrying out a dynamically linked app's Plan (be-wasm-dylink) on an isolate, as
//! crates/be-host/web/dylink.js does in the browser: the root library is instantiated first, the
//! memory grown to hold the rest, and each library and then the app instantiated against
//! `WebAssembly.Global`s for their addresses and thread-locals, the root's function table, and
//! the functions earlier modules export.

use std::sync::{Arc, Mutex};

use be_wasm_dylink::{
    APPLY_DATA_RELOCS, CONSTRUCTORS, Described, INIT_TLS, Import, PAGE_SIZE, Plan, STACK_POINTER,
    TABLE, TLS_BASE,
};

use crate::engine::{self, Kind, Module};
use crate::instance::{self, Answers};
use crate::state::View;

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

pub(crate) fn libraries(module: &engine::Shape) -> Result<Vec<String>, String> {
    be_wasm_dylink::libraries(module.exports.iter().map(|export| export.name.as_str()))
}

pub(crate) struct Linked {
    pub(crate) plan: Plan,
    free: Mutex<Vec<u32>>,
}

impl Linked {
    /// A thread-local block for a new thread, growing the memory by `grow` for more when none is
    /// free.
    pub(crate) fn take(
        &self,
        store: &v8::SharedRef<v8::BackingStore>,
        grow: &mut dyn FnMut(u32) -> Result<u32, String>,
    ) -> Result<u32, String> {
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
            let start = grow(pages).map_err(|error| {
                format!("the memory could not grow for thread-local storage: {error}")
            })?;
            let start = start
                .checked_mul(PAGE_SIZE)
                .ok_or("thread-local storage lies past a 32-bit address space")?;
            free.extend(blocks.carve(start, pages).into_iter().rev());
        }
        let block = free
            .pop()
            .ok_or("no thread-local block fits in the pages grown for them")?;
        View::of(store)
            .write(block, &vec![0; blocks.size as usize])
            .map_err(|_| "thread-local storage ran past the end of the shared memory")?;
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
            .shape
            .imports
            .iter()
            .map(|import| (import.module.clone(), import.name.clone()))
            .collect(),
        ..Described::default()
    };
    for export in &module.shape.exports {
        match export.kind {
            Kind::Function | Kind::Tag => {
                described.functions.insert(export.name.clone());
            }
            Kind::Global => {
                described.globals.insert(export.name.clone());
            }
            Kind::Table | Kind::Memory(_) => {}
        }
        described.exports.push(export.name.clone());
    }
    described
}

fn exported_i32(
    scope: &mut v8::PinScope<'_, '_>,
    exports: v8::Local<v8::Object>,
    name: &str,
) -> Option<i32> {
    let global = engine::get(scope, exports, name)?;
    engine::global_value(scope, global)
}

fn table<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    root: v8::Local<v8::Object>,
) -> Result<v8::Local<'s, v8::Object>, String> {
    engine::get(scope, root, TABLE)
        .and_then(|table| table.to_object(scope))
        .ok_or_else(|| format!("the root library does not export {TABLE}"))
}

fn name(modules: &[Module], index: usize, identities: &[String]) -> String {
    match identities.get(index) {
        _ if index + 1 == modules.len() => "the app".to_owned(),
        Some(identity) => format!("the library {identity}"),
        None => format!("module {index}"),
    }
}

pub(crate) fn plan(
    scope: &mut v8::PinScope<'_, '_>,
    modules: &[Module],
    identities: &[String],
    root: v8::Local<v8::Object>,
    memory: v8::Local<v8::Object>,
) -> Result<Arc<Linked>, String> {
    let described: Vec<Described> = modules
        .iter()
        .enumerate()
        .map(|(index, module)| describe(module, name(modules, index, identities)))
        .collect();
    let pages = be_wasm_dylink::pages(&described)?;
    let placed = crate::threads::grow(scope, memory, pages)
        .map_err(|error| format!("the memory could not grow to hold the app: {error}"))?;
    let placed = placed
        .checked_mul(PAGE_SIZE)
        .ok_or("the app lies past a 32-bit address space")?;
    let table = table(scope, root)?;
    let size = engine::get(scope, table, "length")
        .and_then(|length| length.uint32_value(scope))
        .ok_or("the function table has no length")?;
    Ok(Arc::new(Linked {
        plan: be_wasm_dylink::plan(&described, placed, u64::from(size))?,
        free: Mutex::new(Vec::new()),
    }))
}

pub(crate) struct Thread {
    pub(crate) blocks: u32,
    pub(crate) root_block: u32,
    pub(crate) main: bool,
}

fn fill_slots(
    scope: &mut v8::PinScope<'_, '_>,
    table: v8::Local<v8::Object>,
    plan: &Plan,
    exports: v8::Local<v8::Object>,
    module: usize,
) -> Result<(), String> {
    for slot in plan.slots.iter().filter(|slot| slot.module == module) {
        let function = instance::function(scope, exports, &slot.name).ok_or_else(|| {
            format!(
                "the address of {} is taken, and nothing exports it",
                slot.name
            )
        })?;
        let index = engine::number(scope, slot.slot as f64);
        engine::call(scope, table, "set", &[index, function.into()])
            .ok_or_else(|| format!("{} could not be put in the function table", slot.name))?;
    }
    Ok(())
}

fn unexported(name: &str) -> String {
    format!("{name} is imported, and the module the plan names does not export it")
}

fn called(result: Option<Result<v8::Local<v8::Value>, ()>>, failure: &str) -> Result<(), String> {
    match result {
        None | Some(Ok(_)) => Ok(()),
        Some(Err(())) => Err(failure.to_owned()),
    }
}

/// Instantiates every module after the root against it, as the plan says, and answers the app's
/// exports.
pub(crate) fn link<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    answers: &Answers<'s>,
    modules: &[Module],
    linked: &Linked,
    root: v8::Local<'s, v8::Object>,
    thread: &Thread,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let plan = &linked.plan;
    let table = table(scope, root)?;
    let grown = engine::number(scope, f64::from(plan.table_grow as u32));
    engine::call(scope, table, "grow", &[grown])
        .ok_or("the function table could not grow for the app")?;
    fill_slots(scope, table, plan, root, 0)?;
    let stack_pointer = engine::get(scope, root, STACK_POINTER)
        .ok_or_else(|| format!("the root library does not export {STACK_POINTER}"))?;
    let mut instances = vec![root];
    for (index, module) in modules.iter().enumerate().skip(1) {
        let placed = &plan.modules[index];
        let mut imports = Vec::new();
        let mut own = Vec::new();
        let mut thread_locals = Vec::new();
        for (import, resolution) in module.shape.imports.iter().zip(&placed.imports) {
            let name = import.name.as_str();
            let resolved: v8::Local<v8::Value> = match *resolution {
                Import::Table => table.into(),
                Import::StackPointer => stack_pointer,
                Import::Constant(value) => engine::global(scope, value, false)?,
                Import::FunctionSlot(value) => engine::global(scope, value, true)?,
                Import::Function(definer) => engine::get(scope, instances[definer], name)
                    .filter(|export| export.is_function() || export.is_object())
                    .ok_or_else(|| unexported(name))?,
                Import::Address(definer) if definer < index => {
                    let value = exported_i32(scope, instances[definer], name)
                        .ok_or_else(|| unexported(name))?;
                    engine::global(scope, plan.address(definer, value)? as i32, true)?
                }
                Import::Address(_) => {
                    let entry = engine::global(scope, 0, true)?;
                    own.push((name.to_owned(), entry));
                    entry
                }
                Import::ThreadLocal(definer) => {
                    let entry = engine::global(scope, 0, true)?;
                    thread_locals.push((name.to_owned(), definer, entry));
                    entry
                }
                Import::Elsewhere => answers.answer(scope, import)?,
            };
            imports.push((import, resolved));
        }
        let exports = instance::instantiate(scope, module, imports)?;
        for (name, entry) in own {
            let value = exported_i32(scope, exports, &name).ok_or_else(|| unexported(&name))?;
            engine::set_global_value(scope, entry, plan.address(index, value)? as i32)
                .ok_or_else(|| format!("{name} could not be relocated"))?;
        }
        fill_slots(scope, table, plan, exports, index)?;
        if thread.main {
            called(
                instance::call_export(scope, exports, APPLY_DATA_RELOCS, &[]),
                "the app's data could not be relocated",
            )?;
        }
        let block = v8::Integer::new_from_unsigned(scope, plan.block(thread.blocks, index)).into();
        called(
            instance::call_export(scope, exports, INIT_TLS, &[block]),
            "thread-local storage could not be set up",
        )?;
        let own_base = exported_i32(scope, exports, TLS_BASE).unwrap_or(0) as u32;
        for (name, definer, entry) in thread_locals {
            let base = match definer {
                0 => thread.root_block,
                _ => exported_i32(scope, instances[definer], TLS_BASE).unwrap_or(0) as u32,
            };
            let offset =
                exported_i32(scope, instances[definer], &name).ok_or_else(|| unexported(&name))?;
            let slot =
                be_wasm_dylink::thread_local_slot(base.wrapping_add_signed(offset), own_base);
            engine::set_global_value(scope, entry, slot)
                .ok_or_else(|| format!("the thread-local {name} could not be placed"))?;
        }
        if thread.main {
            called(
                instance::call_export(scope, exports, CONSTRUCTORS, &[]),
                "the app could not run its constructors",
            )?;
        }
        instances.push(exports);
    }
    instances
        .pop()
        .ok_or_else(|| "there is no module to link".to_owned())
}

pub(crate) fn main_thread_locals(
    scope: &mut v8::PinScope<'_, '_>,
    root: v8::Local<v8::Object>,
) -> Result<u32, String> {
    exported_i32(scope, root, TLS_BASE)
        .map(|base| base as u32)
        .ok_or_else(|| format!("the root library does not export {TLS_BASE}"))
}

pub(crate) fn spawned_thread_locals(
    store: &v8::SharedRef<v8::BackingStore>,
    start_argument: i32,
) -> Result<u32, String> {
    let at = be_wasm_dylink::spawned_thread_locals(start_argument)?;
    let bytes = View::of(store).read(at, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
