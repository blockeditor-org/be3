// Linking an app to the libraries it needs, in a browser.
//
// A linked app (guides/app_libraries.md) names the libraries in its closure,
// in dependency order, by the SHA-256 of each module: the first is the root,
// linked at fixed addresses with std and wasi-libc, and each one after it, the
// app last, is a position-independent module that imports what the modules
// below it have: their functions by name from "env", the addresses of their
// data through "GOT.mem" and the table slots of their functions through
// "GOT.func". Where everything goes is planned by crates/be-wasm-dylink, the
// same plan crates/be-wasm-host carries out natively on V8 through the same
// WebAssembly API, called from Rust. The app worker reaches the planner
// through be-wasm-gpu-shim; this only carries the plan out. A guest is
// single-threaded (decisions/guests_are_single_threaded.md), so it is linked
// once, on its one thread.

const TABLE = "__indirect_function_table";
const STACK_POINTER = "__stack_pointer";
const PAGE_SIZE = 65536;

function names(entries) {
    return entries.map(({ name }) => name);
}

/** The libraries a module needs, root first; none for a standalone module. */
export function libraries(shim, module) {
    return shim.libraries(names(WebAssembly.Module.exports(module)));
}

/**
 * Fetches and compiles the library named identity from beside near, refusing
 * one whose bytes do not hash to its name. The compile streams, which lets the
 * browser keep the compiled code for the next worker that fetches the same
 * library.
 */
export async function fetchLibrary(shim, identity, near) {
    const url = new URL(shim.library_file_name(identity), near);
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`the library ${url} could not be fetched: ${response.status}`);
    }
    const [bytes, compiled] = await Promise.all([
        response.clone().arrayBuffer(),
        WebAssembly.compileStreaming(response).catch(() => null),
    ]);
    shim.check_library(new Uint8Array(bytes), identity);
    return { bytes, module: compiled ?? (await WebAssembly.compile(bytes)) };
}

function describe(module, name) {
    const functions = [];
    const globals = [];
    const exports = WebAssembly.Module.exports(module);
    for (const { name: exported, kind } of exports) {
        if (kind === "function" || kind === "tag") {
            functions.push(exported);
        } else if (kind === "global") {
            globals.push(exported);
        }
    }
    return {
        name,
        imports: WebAssembly.Module.imports(module).map(({ module: from, name: imported }) => [from, imported]),
        functions,
        globals,
        exports: names(exports),
    };
}

/**
 * Lays out every module above the root: its data in pages the memory grows by
 * before any of its code runs, its functions in the root's table after the
 * root's own, and a slot for every function an address is taken of.
 */
export function plan(shim, modules, identities, root, memory) {
    const described = JSON.stringify(
        modules.map((module, index) =>
            describe(module, index === modules.length - 1 ? "the app" : `the library ${identities[index]}`),
        ),
    );
    const placed = memory.grow(shim.link_pages(described)) * PAGE_SIZE;
    return JSON.parse(shim.link_plan(described, placed, root.exports[TABLE].length));
}

function constant(value, mutable) {
    return new WebAssembly.Global({ value: "i32", mutable }, value);
}

function exported(instance, name) {
    const value = instance.exports[name];
    if (value === undefined) {
        throw new Error(`${name} is imported, and the module the plan names does not export it`);
    }
    return value;
}

function fillSlots(plan, table, instance, module) {
    for (const { slot, module: definer, name } of plan.slots) {
        if (definer === module) {
            table.set(slot, exported(instance, name));
        }
    }
}

/**
 * Links every module above the root, by the plan: relocates each one's data
 * and runs its constructors. host(module) gives the imports neither the plan
 * nor the other modules answer: WASI, the be3 abi and the memory. Returns the
 * app's instance.
 */
export function link(modules, plan, root, host) {
    const table = root.exports[TABLE];
    table.grow(plan.tableGrow);
    fillSlots(plan, table, root, 0);
    const instances = [root];
    modules.slice(1).forEach((module, at) => {
        const index = at + 1;
        const placed = plan.modules[index];
        const imports = host(module);
        const own = [];
        WebAssembly.Module.imports(module).forEach(({ module: from, name }, position) => {
            const group = (imports[from] ??= { __proto__: null });
            const { kind, value } = placed.imports[position];
            if (kind === "table") {
                group[name] = table;
            } else if (kind === "stackPointer") {
                group[name] = root.exports[STACK_POINTER];
            } else if (kind === "constant") {
                group[name] = constant(value, false);
            } else if (kind === "functionSlot") {
                group[name] = constant(value, true);
            } else if (kind === "function") {
                group[name] = exported(instances[value], name);
            } else if (kind === "address") {
                group[name] = constant(0, true);
                if (value < index) {
                    group[name].value = plan.modules[value].memoryBase + exported(instances[value], name).value;
                } else {
                    own.push([name, group[name]]);
                }
            }
        });
        const instance = new WebAssembly.Instance(module, imports);
        for (const [name, global] of own) {
            global.value = placed.memoryBase + exported(instance, name).value;
        }
        fillSlots(plan, table, instance, index);
        instance.exports.__wasm_apply_data_relocs?.();
        instance.exports.__wasm_call_ctors?.();
        instances.push(instance);
    });
    return instances[instances.length - 1];
}
