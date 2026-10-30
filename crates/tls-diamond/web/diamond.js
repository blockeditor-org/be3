// The thread-local diamond (crates/tls-diamond) in a browser: base, left and
// right linked under the plugin, each with its own thread-local block on every
// thread, and the other modules' thread-locals reached through GOT.tls
// (guides/plugin_sdk.md, "Thread-locals"). This worker is the plugin's main
// thread, and a Worker of the same script each thread it spawns.
//
// Only the thread-local part is block-app's own (dylink.js); the rest is a
// small loader for a list of modules that only this test uses, until the
// plugin loaders link any number of libraries.

import * as dylink from "./dylink.js";
import * as wasi from "./wasi.js";

const NAMES = ["base", "left", "right", "plugin"];
const PAGES = 256;
const MAXIMUM_PAGES = 16384;
const START_ARGUMENT_TLS = 4;
const report = new BroadcastChannel("tls-diamond");

function aligned(value, align) {
    return Math.ceil(value / align) * align;
}

function layout(module) {
    const name = WebAssembly.Module.exports(module)
        .map(({ name }) => name)
        .find((name) => name.startsWith("__be3_dylink_"));
    const [memorySize, memoryAlign, tableSize, tableAlign] = name
        .slice("__be3_dylink_".length)
        .split("_")
        .map(Number);
    return { memorySize, memoryAlign: 2 ** memoryAlign, tableSize, tableAlign: 2 ** tableAlign };
}

function allocate(base, size, align) {
    const alignment = Math.max(align, 1);
    return aligned(base.exports.malloc(Math.max(size, 1) + alignment) >>> 0, alignment);
}

// The base module: the memory's allocator, stack and table, and libc's threads.
function instantiateBase(module, memory, spawn) {
    const imports = { env: { memory }, wasi: { "thread-spawn": spawn }, wasi_snapshot_preview1: {} };
    for (const { module: from, name } of WebAssembly.Module.imports(module)) {
        if (from === "wasi_snapshot_preview1") {
            imports[from][name] = wasi[name] ?? (() => 52);
        }
    }
    return new WebAssembly.Instance(module, imports);
}

// Links the modules above the base for one thread. The first call, on the main
// thread, makes the plan the others follow and allocates the main thread's
// blocks; blocks are where this thread's thread-locals go.
function link(modules, instances, memory, plan, blocks) {
    const [base] = instances;
    const table = base.exports.__indirect_function_table;
    const first = plan.memoryBases.length === 0;
    if (first) {
        let next = table.length;
        for (const module of modules.slice(1)) {
            const { memorySize, memoryAlign, tableSize, tableAlign } = layout(module);
            const memoryBase = allocate(base, memorySize, memoryAlign);
            new Uint8Array(memory.buffer, memoryBase, memorySize).fill(0);
            plan.memoryBases.push(memoryBase);
            const tableBase = aligned(next, tableAlign);
            plan.tableBases.push(tableBase);
            next = tableBase + tableSize;
        }
        plan.slots = next;
        for (const module of modules.slice(1)) {
            next += WebAssembly.Module.imports(module).filter(({ module }) => module === "GOT.func").length;
        }
        plan.tableGrow = next - table.length;
    }
    table.grow(plan.tableGrow);
    let slot = plan.slots;
    const address = (instance, index, name) => {
        const value = instance.exports[name].value;
        return index === 0 ? value : plan.memoryBases[index - 1] + value;
    };
    const definer = (name, kind) => {
        const index = instances.findIndex((instance) => kind(instance.exports[name]));
        if (index < 0) {
            throw new Error(`nothing linked exports ${name}`);
        }
        return index;
    };
    const isGlobal = (value) => value instanceof WebAssembly.Global;
    const isFunction = (value) => typeof value === "function";
    modules.slice(1).forEach((module, at) => {
        const index = at + 1;
        const memoryBase = plan.memoryBases[at];
        const env = {
            memory,
            __indirect_function_table: table,
            __stack_pointer: base.exports.__stack_pointer,
            __memory_base: new WebAssembly.Global({ value: "i32" }, memoryBase),
            __table_base: new WebAssembly.Global({ value: "i32" }, plan.tableBases[at]),
        };
        const memoryGot = {};
        const functionGot = {};
        const own = [];
        for (const { module: from, name } of WebAssembly.Module.imports(module)) {
            if (from === "env" && !(name in env)) {
                const exporter = instances[definer(name, isFunction)];
                env[name] = exporter.exports[name];
            } else if (from === "GOT.mem") {
                const found = instances.findIndex((instance) => isGlobal(instance.exports[name]));
                memoryGot[name] = new WebAssembly.Global({ value: "i32", mutable: true }, 0);
                if (found < 0) {
                    own.push(() => (memoryGot[name].value = memoryBase + instance.exports[name].value));
                } else {
                    memoryGot[name].value = address(instances[found], found, name);
                }
            } else if (from === "GOT.func") {
                const taken = slot++;
                functionGot[name] = new WebAssembly.Global({ value: "i32", mutable: true }, taken);
                const found = instances.findIndex((instance) => isFunction(instance.exports[name]));
                if (found < 0) {
                    own.push(() => table.set(taken, instance.exports[name]));
                } else {
                    table.set(taken, instances[found].exports[name]);
                }
            }
        }
        const threadLocals = dylink.threadLocalImports(module);
        const instance = new WebAssembly.Instance(module, {
            env,
            "GOT.mem": memoryGot,
            "GOT.func": functionGot,
            "GOT.tls": threadLocals,
        });
        own.forEach((fill) => fill());
        if (first) {
            instance.exports.__wasm_apply_data_relocs?.();
            const size = instance.exports.__tls_size.value;
            const align = instance.exports.__tls_align.value;
            blocks.push(allocate(base, size, align));
        }
        instance.exports.__wasm_init_tls(blocks[at]);
        dylink.placeOtherThreadLocals(threadLocals, instance.exports.__tls_base.value, (name) => {
            const found = definer(name, isGlobal);
            const block = found === 0 ? plan.baseBlock : instances[found].exports.__tls_base.value;
            return block + instances[found].exports[name].value;
        });
        instances.push(instance);
        if (first) {
            instance.exports.__wasm_call_ctors?.();
        }
    });
    if (first) {
        plan.sizes = instances.slice(1).map((instance) => [
            instance.exports.__tls_size.value,
            instance.exports.__tls_align.value,
        ]);
    }
}

let nextThread = 1;

// wasi's thread-spawn, on the thread that spawns: it allocates the new
// thread's blocks for every module above the base before the thread runs.
function spawner(modules, memory, plan, base) {
    return (startArgument) => {
        const blocks = plan.sizes.map(([size, align]) => allocate(base(), size, align));
        const id = nextThread++;
        const worker = new Worker(new URL(import.meta.url), { type: "module" });
        worker.postMessage({ modules, memory, plan, blocks, id, startArgument });
        return id;
    };
}

function main(modules) {
    const memory = new WebAssembly.Memory({ initial: PAGES, maximum: MAXIMUM_PAGES, shared: true });
    wasi.bindMemory(memory);
    const plan = { memoryBases: [], tableBases: [] };
    let base;
    base = instantiateBase(modules[0], memory, spawner(modules, memory, plan, () => base));
    base.exports.__wasm_call_ctors();
    base.exports.plugin_initialize_tls(base.exports.__tls_size.value, base.exports.__tls_align.value);
    plan.baseBlock = base.exports.__tls_base.value;
    const instances = [base];
    link(modules, instances, memory, plan, []);
    const plugin = instances[instances.length - 1];
    const reached = WebAssembly.Module.imports(modules[modules.length - 1])
        .filter(({ module }) => module === "GOT.tls")
        .map(({ name }) => name);
    for (const local of ["BASE", "LEFT", "RIGHT"]) {
        if (!reached.some((name) => name.includes(local))) {
            throw new Error(`the plugin does not reach ${local} through GOT.tls: ${reached}`);
        }
    }
    plugin.exports.diamond_start();
    return plugin;
}

// The main thread may not block on the spawned thread while that thread's
// Worker is still starting, which needs this one's event loop.
async function finished(plugin) {
    while (!plugin.exports.diamond_spawned_ended()) {
        await new Promise((resolve) => setTimeout(resolve, 10));
    }
    return plugin.exports.diamond_finish();
}

function thread({ modules, memory, plan, blocks, id, startArgument }) {
    wasi.bindMemory(memory);
    let base;
    base = instantiateBase(modules[0], memory, spawner(modules, memory, plan, () => base));
    const threadPlan = { ...plan, baseBlock: new DataView(memory.buffer).getUint32(startArgument + START_ARGUMENT_TLS, true) };
    link(modules, [base], memory, threadPlan, blocks);
    base.exports.wasi_thread_start(id, startArgument);
}

self.onmessage = async ({ data }) => {
    try {
        if (data.kind === "start") {
            const modules = await Promise.all(
                NAMES.map((name) => WebAssembly.compileStreaming(fetch(`${name}.wasm`))),
            );
            report.postMessage({ result: await finished(main(modules)) });
        } else {
            thread(data);
        }
    } catch (error) {
        report.postMessage({ error: String(error?.stack ?? error) });
    }
};
