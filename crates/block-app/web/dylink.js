// Linking a plugin to the plugin SDK, in a browser.
//
// A plugin built against the plugin SDK (guides/plugin_sdk.md) is a
// position-independent module that imports everything the SDK has: its
// functions by name from "env", the addresses of its data through "GOT.mem"
// and the table slots of its functions through "GOT.func". This instantiates
// one against an SDK instance that shares its memory. Where everything goes is
// planned by crates/wasm-dylink, the same plan crates/block-wasm-host follows
// under wasmtime, which the plugin worker reaches through block-gpu-shim; this
// only carries the plan out. The plugin worker links once (plan, instantiate,
// relocate) and every thread it spawns links again from the same plan, which
// is plain data.

const TABLE = "__indirect_function_table";
const STACK_POINTER = "__stack_pointer";

// wasi-libc's start_args, which a spawned thread is handed: the stack, then
// the thread's thread-local block (START_ARGUMENT_TLS in crates/wasm-dylink).
const START_ARGUMENT_TLS = 4;

function names(entries) {
    return entries.map(({ name }) => name);
}

/** The SDK a module was linked against, or null for a module that has none. */
export function sdkIdentity(shim, module) {
    return shim.sdk_identity(names(WebAssembly.Module.exports(module))) ?? null;
}

const PAGE_SIZE = 65536;

/**
 * Lays the plugin out beside the SDK: its data in pages the memory grows by
 * for it before any of its code runs, its functions in the SDK's table after
 * the SDK's own, and a slot for every function it takes the address of.
 */
export function plan(shim, sdk, plugin, memory) {
    const exports = names(WebAssembly.Module.exports(plugin));
    const placed = memory.grow(shim.link_pages(exports)) * PAGE_SIZE;
    const imports = WebAssembly.Module.imports(plugin);
    const functions = [];
    const globals = [];
    const addresses = [];
    for (const [name, value] of Object.entries(sdk.exports)) {
        if (typeof value === "function") {
            functions.push(name);
        } else if (value instanceof WebAssembly.Global && Number.isInteger(value.value)) {
            globals.push(name);
            addresses.push(value.value);
        }
    }
    const layout = JSON.parse(
        shim.link_plan(
            exports,
            imports.map(({ module }) => module),
            names(imports),
            functions,
            globals,
            Int32Array.from(addresses),
            placed,
            sdk.exports[TABLE].length,
        ),
    );
    return layout;
}

function constant(value, mutable) {
    return new WebAssembly.Global({ value: "i32", mutable }, value);
}

/**
 * Instantiates the plugin against the SDK by the plan. host answers what
 * neither the SDK nor the plan does: WASI, the be3 abi and the memory.
 */
export function instantiate(sdk, plugin, layout, host) {
    const table = sdk.exports[TABLE];
    table.grow(layout.tableGrow);
    for (const [slot, name] of layout.sdkSlots) {
        table.set(slot, sdk.exports[name]);
    }
    const imports = { __proto__: null };
    const own = [];
    WebAssembly.Module.imports(plugin).forEach(({ module, name, kind: type }, index) => {
        const group = (imports[module] ??= { __proto__: null });
        const { kind, value } = layout.imports[index];
        if (kind === "table") {
            group[name] = table;
        } else if (kind === "stackpointer") {
            group[name] = sdk.exports[STACK_POINTER];
        } else if (kind === "constant") {
            group[name] = constant(value, false);
        } else if (kind === "slot" || kind === "address") {
            group[name] = constant(value, true);
        } else if (kind === "own") {
            group[name] = constant(0, true);
            own.push([module, name]);
        } else if (module !== "env" || type === "memory") {
            group[name] = host[module]?.[name];
        } else if (typeof sdk.exports[name] === "function" || sdk.exports[name] instanceof WebAssembly.Tag) {
            group[name] = sdk.exports[name];
        } else {
            throw new Error(`the plugin imports ${name}, which the plugin SDK does not export`);
        }
    });
    const instance = new WebAssembly.Instance(plugin, imports);
    for (const [module, name] of own) {
        imports[module][name].value = layout.memoryBase + instance.exports[name].value;
    }
    for (const [slot, name] of layout.pluginSlots) {
        table.set(slot, instance.exports[name]);
    }
    return instance;
}

/**
 * What only the first instance does: notes where the plugin's own
 * thread-locals start out, and fixes the addresses in its data.
 */
export function relocate(instance, layout) {
    layout.image = instance.exports.__tls_base.value;
    layout.tlsSize = instance.exports.__tls_size.value;
    instance.exports.__wasm_apply_data_relocs();
}

/**
 * Points the plugin's thread-locals at the SDK's block for this thread, whose
 * tail the SDK reserves for the plugin's own, and copies their first values in.
 */
export function placeThreadLocals(instance, memory, layout, base) {
    if (layout.tlsSize > layout.reserve) {
        const bytes = new Uint8Array(memory.buffer);
        bytes.copyWithin(base + layout.reserve, layout.image + layout.reserve, layout.image + layout.tlsSize);
    }
    instance.exports.__tls_base.value = base;
}

/** The thread-local block wasi-libc laid out for the thread being spawned. */
export function spawnedThreadLocals(memory, startArgument) {
    return new DataView(memory.buffer).getUint32(startArgument + START_ARGUMENT_TLS, true);
}

// A thread-local another module defines is reached through an imported
// global, GOT.tls.<symbol>, which wasm-link puts where the code had the
// symbol's offset from its own __tls_base (guides/plugin_sdk.md,
// "Thread-locals"). Every thread instantiates every module again, so the
// global is per thread: it holds where the symbol lies in this thread's block
// of the module that defines it, less this thread's block of the module that
// imports it, since the code still adds its own __tls_base.
const GOT_THREAD_LOCAL = "GOT.tls";

/** A mutable global for each GOT.tls import of a module, to put in its imports. */
export function threadLocalImports(module) {
    const globals = { __proto__: null };
    for (const { module: from, name } of WebAssembly.Module.imports(module)) {
        if (from === GOT_THREAD_LOCAL) {
            globals[name] = new WebAssembly.Global({ value: "i32", mutable: true }, 0);
        }
    }
    return globals;
}

/**
 * Points a module's GOT.tls globals at this thread's copies. own is this
 * thread's block of the module, and address(name) where the symbol lies in this
 * thread's block of the module that defines it.
 */
export function placeOtherThreadLocals(globals, own, address) {
    for (const [name, global] of Object.entries(globals)) {
        global.value = (address(name) - own) | 0;
    }
}
