use super::*;

use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicI32, Ordering},
};

use wasmtime::{
    Caller, Config, Engine, Extern, Global, GlobalType, Instance, Linker, Module, Mutability, Ref,
    SharedMemory, Store, Val, ValType,
};
use wasmtime_wasi::{WasiCtxBuilder, p1};

const BASE: &[u8] = include_bytes!(env!("TLS_DIAMOND_BASE_WASM"));
const LEFT: &[u8] = include_bytes!(env!("TLS_DIAMOND_LEFT_WASM"));
const RIGHT: &[u8] = include_bytes!(env!("TLS_DIAMOND_RIGHT_WASM"));
const PLUGIN: &[u8] = include_bytes!(env!("TLS_DIAMOND_PLUGIN_WASM"));

type Data = p1::WasiP1Ctx;

#[derive(Default)]
struct Plan {
    memory_bases: Vec<u32>,
    table_bases: Vec<u32>,
    slots: u64,
    table_grow: u64,
    sizes: Vec<(u32, u32)>,
}

struct Shared {
    engine: Engine,
    modules: Vec<Module>,
    memory: SharedMemory,
    plan: OnceLock<Plan>,
    next: AtomicI32,
    threads: Mutex<Vec<std::thread::JoinHandle<Result<(), String>>>>,
}

fn failed(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn aligned(value: u64, align: u64) -> u64 {
    value.div_ceil(align) * align
}

fn allocate(
    malloc: &wasmtime::TypedFunc<u32, u32>,
    store: impl wasmtime::AsContextMut,
    size: u32,
    align: u32,
) -> Result<u32, String> {
    let align = align.max(1);
    let at = malloc.call(store, size.max(1) + align).map_err(failed)?;
    Ok(aligned(u64::from(at), u64::from(align)) as u32)
}

fn i32_export(instance: &Instance, store: &mut Store<Data>, name: &str) -> Result<u32, String> {
    instance
        .get_global(&mut *store, name)
        .and_then(|global| global.get(&mut *store).i32())
        .map(|value| value as u32)
        .ok_or_else(|| format!("nothing exports {name}"))
}

fn constant(store: &mut Store<Data>, value: u32, mutability: Mutability) -> Result<Global, String> {
    Global::new(
        &mut *store,
        GlobalType::new(ValType::I32, mutability),
        Val::I32(value as i32),
    )
    .map_err(failed)
}

fn instantiate_base(shared: &Arc<Shared>, store: &mut Store<Data>) -> Result<Instance, String> {
    let mut linker: Linker<Data> = Linker::new(&shared.engine);
    p1::add_to_linker_sync(&mut linker, |data: &mut Data| data).map_err(failed)?;
    linker
        .define(&mut *store, "env", "memory", shared.memory.clone())
        .map_err(failed)?;
    let spawner = Arc::clone(shared);
    linker
        .func_wrap(
            "wasi",
            "thread-spawn",
            move |caller: Caller<'_, Data>, argument: i32| -> i32 {
                spawn(&spawner, caller, argument).unwrap_or_else(|error| {
                    eprintln!("{error}");
                    -1
                })
            },
        )
        .map_err(failed)?;
    linker
        .instantiate(&mut *store, &shared.modules[0])
        .map_err(failed)
}

fn spawn(shared: &Arc<Shared>, mut caller: Caller<'_, Data>, argument: i32) -> Result<i32, String> {
    let plan = shared
        .plan
        .get()
        .ok_or("spawned before the plugin was linked")?;
    let malloc = caller
        .get_export("malloc")
        .and_then(Extern::into_func)
        .ok_or("the base exports no malloc")?
        .typed::<u32, u32>(&caller)
        .map_err(failed)?;
    let blocks = plan
        .sizes
        .iter()
        .map(|(size, align)| allocate(&malloc, &mut caller, *size, *align))
        .collect::<Result<Vec<_>, _>>()?;
    let thread = shared.next.fetch_add(1, Ordering::Relaxed);
    let started = Arc::clone(shared);
    let handle = std::thread::spawn(move || {
        let mut store = Store::new(
            &started.engine,
            WasiCtxBuilder::new().inherit_stderr().build_p1(),
        );
        let base = instantiate_base(&started, &mut store)?;
        let base_block = dylink::spawned_thread_locals(&started.memory, argument)?;
        let plan = started.plan.get().ok_or("the plan went missing")?;
        link(&started, &mut store, vec![base], plan, &blocks, base_block)?;
        base.get_typed_func::<(i32, i32), ()>(&mut store, "wasi_thread_start")
            .map_err(failed)?
            .call(&mut store, (thread, argument))
            .map_err(|error| format!("the spawned thread trapped: {error:?}"))
    });
    shared.threads.lock().map_err(failed)?.push(handle);
    Ok(thread)
}

fn plan(shared: &Shared, store: &mut Store<Data>, base: &Instance) -> Result<Plan, String> {
    let malloc = base
        .get_typed_func::<u32, u32>(&mut *store, "malloc")
        .map_err(failed)?;
    let table = base
        .get_table(&mut *store, "__indirect_function_table")
        .ok_or("the base exports no table")?;
    let mut plan = Plan::default();
    let mut next = table.size(&mut *store);
    for module in &shared.modules[1..] {
        let declared = wasm_dylink::Declared::of(module.exports().map(|export| export.name()))?;
        let memory_base = allocate(
            &malloc,
            &mut *store,
            declared.memory_size,
            1 << declared.memory_align,
        )?;
        plan.memory_bases.push(memory_base);
        let table_base = aligned(next, 1 << declared.table_align);
        plan.table_bases.push(table_base as u32);
        next = table_base + u64::from(declared.table_size);
    }
    plan.slots = next;
    for module in &shared.modules[1..] {
        next += module
            .imports()
            .filter(|import| import.module() == "GOT.func")
            .count() as u64;
    }
    plan.table_grow = next - table.size(&mut *store);
    Ok(plan)
}

fn link(
    shared: &Shared,
    store: &mut Store<Data>,
    mut instances: Vec<Instance>,
    plan: &Plan,
    blocks: &[u32],
    base_block: u32,
) -> Result<Vec<Instance>, String> {
    let base = instances[0];
    let first = blocks.is_empty();
    let mut blocks = blocks.to_vec();
    let table = base
        .get_table(&mut *store, "__indirect_function_table")
        .ok_or("the base exports no table")?;
    table
        .grow(&mut *store, plan.table_grow, Ref::Func(None))
        .map_err(failed)?;
    let stack_pointer = base
        .get_global(&mut *store, "__stack_pointer")
        .ok_or("the base exports no stack pointer")?;
    let malloc = base
        .get_typed_func::<u32, u32>(&mut *store, "malloc")
        .map_err(failed)?;
    let mut slot = plan.slots;
    for (at, module) in shared.modules[1..].iter().enumerate() {
        let memory_base = plan.memory_bases[at];
        let thread_locals = dylink::thread_local_imports(store, module)?;
        let mut imports: Vec<Extern> = Vec::new();
        let mut own_addresses = Vec::new();
        let mut own_functions = Vec::new();
        for import in module.imports() {
            let (from, name) = (import.module(), import.name());
            let resolved: Extern = match (from, name) {
                ("env", "memory") => shared.memory.clone().into(),
                ("env", "__indirect_function_table") => table.into(),
                ("env", "__stack_pointer") => stack_pointer.into(),
                ("env", "__memory_base") => constant(store, memory_base, Mutability::Const)?.into(),
                ("env", "__table_base") => {
                    constant(store, plan.table_bases[at], Mutability::Const)?.into()
                }
                ("env", _) => instances
                    .iter()
                    .find_map(|instance| instance.get_func(&mut *store, name))
                    .ok_or_else(|| format!("nothing linked exports {name}"))?
                    .into(),
                ("GOT.mem", _) => {
                    let found = instances
                        .iter()
                        .position(|instance| instance.get_global(&mut *store, name).is_some());
                    let entry = constant(store, 0, Mutability::Var)?;
                    match found {
                        Some(0) => {
                            let address = i32_export(&base, store, name)?;
                            entry
                                .set(&mut *store, Val::I32(address as i32))
                                .map_err(failed)?;
                        }
                        Some(index) => {
                            let offset = i32_export(&instances[index], store, name)?;
                            let address = plan.memory_bases[index - 1] + offset;
                            entry
                                .set(&mut *store, Val::I32(address as i32))
                                .map_err(failed)?;
                        }
                        None => own_addresses.push((name.to_owned(), entry)),
                    }
                    entry.into()
                }
                ("GOT.func", _) => {
                    let taken = slot;
                    slot += 1;
                    let found = instances
                        .iter()
                        .find_map(|instance| instance.get_func(&mut *store, name));
                    match found {
                        Some(function) => table
                            .set(&mut *store, taken, Ref::Func(Some(function)))
                            .map_err(failed)?,
                        None => own_functions.push((name.to_owned(), taken)),
                    }
                    constant(store, taken as u32, Mutability::Var)?.into()
                }
                ("GOT.tls", _) => thread_locals
                    .iter()
                    .find(|(wanted, _)| wanted == name)
                    .map(|(_, global)| *global)
                    .ok_or("a thread-local import went missing")?
                    .into(),
                _ => return Err(format!("nothing answers {from}.{name}")),
            };
            imports.push(resolved);
        }
        let instance = Instance::new(&mut *store, module, &imports).map_err(failed)?;
        for (name, entry) in own_addresses {
            let address = memory_base + i32_export(&instance, store, &name)?;
            entry
                .set(&mut *store, Val::I32(address as i32))
                .map_err(failed)?;
        }
        for (name, taken) in own_functions {
            let function = instance
                .get_func(&mut *store, &name)
                .ok_or_else(|| format!("nothing exports {name}"))?;
            table
                .set(&mut *store, taken, Ref::Func(Some(function)))
                .map_err(failed)?;
        }
        if first {
            if let Ok(relocate) =
                instance.get_typed_func::<(), ()>(&mut *store, "__wasm_apply_data_relocs")
            {
                relocate.call(&mut *store, ()).map_err(failed)?;
            }
            let size = i32_export(&instance, store, "__tls_size")?;
            let align = i32_export(&instance, store, "__tls_align")?;
            blocks.push(allocate(&malloc, &mut *store, size, align)?);
        }
        instance
            .get_typed_func::<u32, ()>(&mut *store, "__wasm_init_tls")
            .map_err(failed)?
            .call(&mut *store, blocks[at])
            .map_err(failed)?;
        let own_base = i32_export(&instance, store, "__tls_base")?;
        dylink::place_other_thread_locals(store, &thread_locals, own_base, |store, name| {
            let found = instances
                .iter()
                .position(|instance| instance.get_global(&mut *store, name).is_some())
                .ok_or_else(|| format!("nothing linked defines the thread-local {name}"))?;
            let block = match found {
                0 => base_block,
                index => i32_export(&instances[index], store, "__tls_base")?,
            };
            Ok(block + i32_export(&instances[found], store, name)?)
        })?;
        instances.push(instance);
        if first
            && let Ok(constructors) =
                instance.get_typed_func::<(), ()>(&mut *store, "__wasm_call_ctors")
        {
            constructors.call(&mut *store, ()).map_err(failed)?;
        }
    }
    Ok(instances)
}

fn sizes(store: &mut Store<Data>, instances: &[Instance]) -> Result<Vec<(u32, u32)>, String> {
    instances[1..]
        .iter()
        .map(|instance| {
            Ok((
                i32_export(instance, store, "__tls_size")?,
                i32_export(instance, store, "__tls_align")?,
            ))
        })
        .collect()
}

#[test]
fn the_thread_local_diamond_runs_on_every_thread() {
    let mut config = Config::new();
    config.wasm_threads(true);
    config.shared_memory(true);
    let engine = Engine::new(&config).unwrap();
    let modules: Vec<Module> = [BASE, LEFT, RIGHT, PLUGIN]
        .into_iter()
        .map(|bytes| Module::new(&engine, bytes).unwrap())
        .collect();
    let reached: Vec<&str> = modules[3]
        .imports()
        .filter(|import| import.module() == "GOT.tls")
        .map(|import| import.name())
        .collect();
    for local in ["BASE", "LEFT", "RIGHT"] {
        assert!(
            reached.iter().any(|name| name.contains(local)),
            "the plugin reaches {local} without GOT.tls: {reached:?}"
        );
    }
    let memory = shared_memory(&engine, &modules[0]).unwrap();
    let shared = Arc::new(Shared {
        engine,
        modules,
        memory,
        plan: OnceLock::new(),
        next: AtomicI32::new(1),
        threads: Mutex::new(Vec::new()),
    });
    let mut store = Store::new(
        &shared.engine,
        WasiCtxBuilder::new().inherit_stderr().build_p1(),
    );
    let base = instantiate_base(&shared, &mut store).unwrap();
    base.get_typed_func::<(), ()>(&mut store, "__wasm_call_ctors")
        .unwrap()
        .call(&mut store, ())
        .unwrap();
    let size = i32_export(&base, &mut store, "__tls_size").unwrap();
    let align = i32_export(&base, &mut store, "__tls_align").unwrap();
    base.get_typed_func::<(u32, u32), ()>(&mut store, "plugin_initialize_tls")
        .unwrap()
        .call(&mut store, (size, align))
        .unwrap();
    let base_block = i32_export(&base, &mut store, "__tls_base").unwrap();
    let mut plan = plan(&shared, &mut store, &base).unwrap();
    let instances = link(&shared, &mut store, vec![base], &plan, &[], base_block).unwrap();
    plan.sizes = sizes(&mut store, &instances).unwrap();
    assert!(shared.plan.set(plan).is_ok());
    let plugin = instances[3];
    plugin
        .get_typed_func::<(), ()>(&mut store, "diamond_start")
        .unwrap()
        .call(&mut store, ())
        .unwrap();
    let finished = plugin
        .get_typed_func::<(), u32>(&mut store, "diamond_finish")
        .unwrap()
        .call(&mut store, ())
        .unwrap();
    assert_eq!(finished, 1);
    for thread in shared.threads.lock().unwrap().drain(..) {
        thread.join().unwrap().unwrap();
    }
}
