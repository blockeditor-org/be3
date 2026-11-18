//! Carrying out a dynamically linked app's Plan (be-wasm-dylink) on an isolate, as
//! crates/be-host/web/dylink.js does in the browser: the root library is instantiated first, the
//! memory grown to hold the rest, and each library and then the app instantiated against
//! `WebAssembly.Global`s for their addresses, the root's function table, and the functions
//! earlier modules export.

use std::sync::Arc;

use be_wasm_dylink::{
    APPLY_DATA_RELOCS, CONSTRUCTORS, Described, Import, PAGE_SIZE, Plan, STACK_POINTER, TABLE,
};

use crate::engine::{self, Kind, Module};
use crate::instance::{self, Answers};

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
) -> Result<Plan, String> {
    let described: Vec<Described> = modules
        .iter()
        .enumerate()
        .map(|(index, module)| describe(module, name(modules, index, identities)))
        .collect();
    let pages = be_wasm_dylink::pages(&described)?;
    let placed = grow(scope, memory, pages)
        .map_err(|error| format!("the memory could not grow to hold the app: {error}"))?;
    let placed = placed
        .checked_mul(PAGE_SIZE)
        .ok_or("the app lies past a 32-bit address space")?;
    let table = table(scope, root)?;
    let size = engine::get(scope, table, "length")
        .and_then(|length| length.uint32_value(scope))
        .ok_or("the function table has no length")?;
    be_wasm_dylink::plan(&described, placed, u64::from(size))
}

/// Grows `memory` by `pages`, as `memory.grow` does, answering the pages it had.
fn grow(
    scope: &mut v8::PinScope<'_, '_>,
    memory: v8::Local<v8::Object>,
    pages: u32,
) -> Result<u32, String> {
    let pages = engine::number(scope, f64::from(pages));
    engine::call(scope, memory, "grow", &[pages])
        .and_then(|had| had.uint32_value(scope))
        .ok_or_else(|| "the app memory could not grow".to_owned())
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
    plan: &Plan,
    root: v8::Local<'s, v8::Object>,
) -> Result<v8::Local<'s, v8::Object>, String> {
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
        called(
            instance::call_export(scope, exports, APPLY_DATA_RELOCS, &[]),
            "the app's data could not be relocated",
        )?;
        called(
            instance::call_export(scope, exports, CONSTRUCTORS, &[]),
            "the app could not run its constructors",
        )?;
        instances.push(exports);
    }
    instances
        .pop()
        .ok_or_else(|| "there is no module to link".to_owned())
}
