// Linking a plugin to the libraries it needs, in a browser.
//
// A linked plugin (guides/plugin_libraries.md) names the libraries in its closure,
// in dependency order, by the SHA-256 of each module: the first is the root,
// linked at fixed addresses with std and wasi-libc, and each one after it, the
// plugin last, is a position-independent module that imports what the modules
// below it have: their functions by name from "env", the addresses of their
// data through "GOT.mem", the table slots of their functions through
// "GOT.func" and their thread-locals through "GOT.tls". Where everything goes
// is planned by crates/wasm-dylink, the same plan crates/block-wasm-host
// follows under wasmtime, which the plugin worker reaches through
// block-gpu-shim; this only carries the plan out. The plugin worker plans
// once, and every thread links the whole list again from the same plan, which
// is plain data, each with thread-local blocks of its own.

const TABLE = "__indirect_function_table";
const STACK_POINTER = "__stack_pointer";
const PAGE_SIZE = 65536;

// wasi-libc's start_args, which a spawned thread is handed: the stack, then
// the thread's thread-local block (START_ARGUMENT_TLS in crates/wasm-dylink).
const START_ARGUMENT_TLS = 4;

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
 * root's own, a slot for every function an address is taken of, and where its
 * thread-local block lies among a thread's.
 */
export function plan(shim, modules, identities, root, memory) {
    const described = JSON.stringify(
        modules.map((module, index) =>
            describe(module, index === modules.length - 1 ? "the plugin" : `the library ${identities[index]}`),
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
 * Links every module above the root, by the plan, on this thread. thread is
 * { blocks, rootBlock, main }: where this thread's blocks for those modules
 * start (takeBlocks), the root's block on this thread, and whether this is the
 * thread that relocates the data and runs the constructors. host(module) gives
 * the imports neither the plan nor the other modules answer: WASI, the be3
 * abi and the memory. Returns the plugin's instance.
 */
export function link(modules, plan, root, thread, host) {
    const table = root.exports[TABLE];
    table.grow(plan.tableGrow);
    fillSlots(plan, table, root, 0);
    const instances = [root];
    modules.slice(1).forEach((module, at) => {
        const index = at + 1;
        const placed = plan.modules[index];
        const imports = host(module);
        const own = [];
        const threadLocals = [];
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
            } else if (kind === "threadLocal") {
                group[name] = constant(0, true);
                threadLocals.push([name, value, group[name]]);
            }
        });
        const instance = new WebAssembly.Instance(module, imports);
        for (const [name, global] of own) {
            global.value = placed.memoryBase + exported(instance, name).value;
        }
        fillSlots(plan, table, instance, index);
        if (thread.main) {
            instance.exports.__wasm_apply_data_relocs?.();
        }
        instance.exports.__wasm_init_tls?.(thread.blocks + placed.block);
        const ownBase = instance.exports.__tls_base?.value ?? 0;
        for (const [name, definer, global] of threadLocals) {
            const base = definer === 0 ? thread.rootBlock : (instances[definer].exports.__tls_base?.value ?? 0);
            global.value = (base + exported(instances[definer], name).value - ownBase) | 0;
        }
        if (thread.main) {
            instance.exports.__wasm_call_ctors?.();
        }
        instances.push(instance);
    });
    return instances[instances.length - 1];
}

// A thread's blocks for the modules above the root come from pages the memory
// grows by for them, carved into blocks of the plan's size. The free ones are a
// list kept in the memory itself, under a lock at plan.region, so that any
// thread can take blocks for a thread it spawns and give back its own when it
// ends.
function locked(memory, plan, body) {
    const words = new Int32Array(memory.buffer);
    const lock = plan.region >>> 2;
    while (Atomics.compareExchange(words, lock, 0, 1) !== 0) {
        Atomics.wait(words, lock, 1, 1);
    }
    try {
        return body(lock + 1);
    } finally {
        Atomics.store(words, lock, 0);
        Atomics.notify(words, lock, 1);
    }
}

/** A thread's blocks, zeroed: where its thread-local blocks for every module above the root lie. */
export function takeBlocks(memory, plan) {
    const { size, align, pages } = plan.blocks;
    if (size === 0) {
        return 0;
    }
    const block = locked(memory, plan, (head) => {
        if (new Int32Array(memory.buffer)[head] === 0) {
            const start = memory.grow(pages) * PAGE_SIZE;
            const end = start + pages * PAGE_SIZE;
            const words = new Int32Array(memory.buffer);
            for (let at = Math.ceil(start / align) * align; at + size <= end; at += size) {
                words[at >>> 2] = words[head];
                words[head] = at;
            }
        }
        const words = new Int32Array(memory.buffer);
        const taken = words[head] >>> 0;
        words[head] = words[taken >>> 2];
        return taken;
    });
    new Uint8Array(memory.buffer, block, size).fill(0);
    return block;
}

/** Gives back the blocks takeBlocks gave a thread that has ended. */
export function freeBlocks(memory, plan, blocks) {
    if (plan.blocks.size === 0) {
        return;
    }
    locked(memory, plan, (head) => {
        const words = new Int32Array(memory.buffer);
        words[blocks >>> 2] = words[head];
        words[head] = blocks;
    });
}

/** The thread-local block wasi-libc laid out for the root on the thread being spawned. */
export function spawnedThreadLocals(memory, startArgument) {
    return new DataView(memory.buffer).getUint32(startArgument + START_ARGUMENT_TLS, true);
}
