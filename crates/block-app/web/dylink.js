// Linking a plugin to the plugin SDK, in a browser.
//
// A plugin built against the plugin SDK (guides/plugin_sdk.md) is a
// position-independent module that imports everything the SDK has: its
// functions by name from "env", the addresses of its data through "GOT.mem"
// and the table slots of its functions through "GOT.func". This instantiates
// one against an SDK instance that shares its memory, the way
// crates/block-wasm-host/src/dylink.rs does under wasmtime, so the same files
// run on both. The plugin worker links once (plan, instantiate, relocate) and
// every thread it spawns links again from the same plan.

const SDK_EXPORT_PREFIX = "__be3_sdk_";
const LAYOUT_EXPORT_PREFIX = "__be3_dylink_";
const PLUGIN_TLS = "__be3_plugin_tls";
const TABLE = "__indirect_function_table";
const STACK_POINTER = "__stack_pointer";
const MAX_ALIGN_LOG2 = 16;

// wasi-libc's start_args, which a spawned thread is handed: the stack, then
// the thread's thread-local block.
const START_ARGUMENT_TLS = 4;

/** The SDK a module was linked against, or null for a module that has none. */
export function sdkIdentity(module) {
    for (const { name } of WebAssembly.Module.exports(module)) {
        if (name.startsWith(SDK_EXPORT_PREFIX)) {
            const identity = name.slice(SDK_EXPORT_PREFIX.length);
            if (!/^[0-9a-f]{64}$/.test(identity)) {
                throw new Error(`the plugin names ${identity} as its plugin SDK, which is not a SHA-256`);
            }
            return identity;
        }
    }
    return null;
}

export function sdkFileName(identity) {
    return `sdk-${identity}.wasm`;
}

function declared(module) {
    for (const { name } of WebAssembly.Module.exports(module)) {
        if (!name.startsWith(LAYOUT_EXPORT_PREFIX)) {
            continue;
        }
        const numbers = name.slice(LAYOUT_EXPORT_PREFIX.length).split("_").map(Number);
        if (numbers.length !== 4 || numbers.some((number) => !Number.isInteger(number))) {
            break;
        }
        const [memorySize, memoryAlign, tableSize, tableAlign] = numbers;
        return {
            memorySize,
            memoryAlign: Math.min(memoryAlign, MAX_ALIGN_LOG2),
            tableSize,
            tableAlign: Math.min(tableAlign, MAX_ALIGN_LOG2),
        };
    }
    throw new Error("the plugin does not say how much memory and table it needs");
}

function aligned(value, alignLog2) {
    const align = 2 ** alignLog2;
    return Math.ceil(value / align) * align;
}

/**
 * Lays the plugin out beside the SDK: its data in memory the SDK allocates,
 * its functions in the SDK's table after the SDK's own, and a slot for every
 * function it takes the address of. The result is plain data, which is how
 * the plugin's threads are told it.
 */
export function plan(sdk, plugin, memory) {
    const wanted = declared(plugin);
    const allocated = sdk.exports.malloc(wanted.memorySize + 2 ** wanted.memoryAlign) >>> 0;
    if (allocated === 0) {
        throw new Error("the plugin SDK could not allocate the plugin's memory");
    }
    const memoryBase = aligned(allocated, wanted.memoryAlign);
    new Uint8Array(memory.buffer, memoryBase, wanted.memorySize).fill(0);
    const size = sdk.exports[TABLE].length;
    const tableBase = aligned(size, wanted.tableAlign);
    let next = tableBase + wanted.tableSize;
    const sdkSlots = [];
    const pluginSlots = [];
    const data = {};
    for (const { module, name } of WebAssembly.Module.imports(plugin)) {
        if (module === "GOT.func") {
            const slots = typeof sdk.exports[name] === "function" ? sdkSlots : pluginSlots;
            slots.push([next, name]);
            next += 1;
        } else if (module === "GOT.mem" && sdk.exports[name] instanceof WebAssembly.Global) {
            data[name] = sdk.exports[name].value;
        }
    }
    return {
        memoryBase,
        tableBase,
        tableGrow: next - size,
        sdkSlots,
        pluginSlots,
        data,
        reserve: sdk.exports[PLUGIN_TLS].value,
        tlsSize: 0,
        image: 0,
    };
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
    const slots = new Map([...layout.sdkSlots, ...layout.pluginSlots].map(([slot, name]) => [name, slot]));
    const imports = { __proto__: null };
    const own = [];
    for (const { module, name, kind } of WebAssembly.Module.imports(plugin)) {
        const group = (imports[module] ??= { __proto__: null });
        if (module === "GOT.func") {
            group[name] = constant(slots.get(name) ?? 0, true);
        } else if (module === "GOT.mem") {
            const address = layout.data[name];
            group[name] = constant(address ?? 0, true);
            if (address === undefined) {
                own.push(name);
            }
        } else if (module !== "env" || kind === "memory") {
            group[name] = host[module]?.[name];
        } else if (name === TABLE) {
            group[name] = table;
        } else if (name === STACK_POINTER) {
            group[name] = sdk.exports[STACK_POINTER];
        } else if (name === "__memory_base") {
            group[name] = constant(layout.memoryBase, false);
        } else if (name === "__table_base") {
            group[name] = constant(layout.tableBase, false);
        } else if (typeof sdk.exports[name] === "function") {
            group[name] = sdk.exports[name];
        } else {
            throw new Error(`the plugin imports ${name}, which the plugin SDK does not export`);
        }
    }
    const instance = new WebAssembly.Instance(plugin, imports);
    for (const name of own) {
        imports["GOT.mem"][name].value = layout.memoryBase + instance.exports[name].value;
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
