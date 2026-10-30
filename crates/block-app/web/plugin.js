// Instantiating a plugin in the browser.
//
// A plugin is the same bindgen-free wasm that wasmtime runs on every other
// platform: its only imports are be3_host, its world's and the be3 gpu abi,
// WASI and wasi-threads if it uses them, and, when it was linked against
// libraries, the libraries, which are fetched from beside it and linked to it
// here (dylink.js). A standalone plugin is instantiated as it is, with the memory it
// imports, shared or not, or the one it exports (guides/plugin_sdk.md, "The
// raw plugin interface"). In a browser the
// abi is answered by block-gpu-shim, a second module that does have bindgen
// and a real wgpu device in the worker, which draws what the plugin shows into
// the canvases the page transferred to the worker. The two modules have
// separate memories, so the shim is handed the plugin's and every import is
// bound straight to the shim's export for it, which reads and writes what a
// pointer points at in the plugin's memory itself: be3_host's under their own
// names, and each import of the gpu world as `module#name` (be-ops-bindgen's
// listing of a world's local imports, which the shim exports). A preview the plugin
// paints leaves the worker as a bitmap of each surface the page exports, and a
// bitmap another worker painted arrives as a texture the plugin imports.
//
// The frames between the host and the plugin are answered here rather than by
// the shim, since they share one ordered stream with the plugin's direct calls
// (guides/type_modules.md): a sync import of the plugin's world is a call item
// the host reads among its frames, and a call item from the host runs the
// export it names once the frames before it were stepped through.

import * as dylink from "./dylink.js";
import * as wasi from "./wasi.js";
import * as threads from "./threads.js";

function stub(module, name) {
    return () => {
        throw new Error(`a plugin called ${module}.${name}, which its host does not answer`);
    };
}

function forward(shim, module, name, exported) {
    const call = shim[exported];
    return typeof call === "function" ? call : stub(module, name);
}

const MARK = [0xff, 0xff, 0xff, 0xff];
const CALL = 1;
const RETURNED = 2;

function marked(item) {
    return item.length >= 5 && MARK.every((byte, index) => item[index] === byte);
}

function callItem(name, hasParent, parent, args) {
    const encoded = new TextEncoder().encode(name);
    const item = new Uint8Array(16 + encoded.length + args.length);
    const view = new DataView(item.buffer);
    item.set(MARK, 0);
    item[4] = CALL;
    item[5] = hasParent ? 1 : 0;
    view.setBigUint64(6, BigInt.asUintN(64, BigInt(parent)));
    view.setUint16(14, encoded.length);
    item.set(encoded, 16);
    item.set(args, 16 + encoded.length);
    return item;
}

function transport(memory, direct) {
    const inbox = [];
    let outbox = [];
    const bytes = () => new Uint8Array(memory().buffer);
    return {
        inbox,
        direct,
        post: (item) => outbox.push(item),
        collect() {
            const frames = outbox;
            outbox = [];
            return frames;
        },
        host_send(pointer, length) {
            outbox.push(bytes().slice(pointer, pointer + length));
        },
        host_receive(pointer, capacity) {
            if (inbox.length === 0) return -1n;
            const frame = inbox[0];
            if (frame.length > capacity) return BigInt(frame.length);
            bytes().set(frame, pointer);
            inbox.shift();
            return BigInt(frame.length);
        },
        forward(name) {
            return (hasParent, parent, pointer, length) => {
                const args = bytes().slice(pointer, pointer + length);
                outbox.push(callItem(name, hasParent !== 0, parent, args));
            };
        },
    };
}

function callExport(exports, memory, item) {
    const view = new DataView(item.buffer, item.byteOffset, item.byteLength);
    const hasParent = item[5] & 1;
    const answered = (item[5] & 2) !== 0;
    const parent = BigInt.asIntN(64, view.getBigUint64(6));
    const length = view.getUint16(14);
    const name = new TextDecoder().decode(item.subarray(16, 16 + length));
    const args = item.subarray(16 + length);
    const exported = exports[name];
    if (typeof exported !== "function") throw new Error(`the plugin does not export ${name}`);
    const pointer = exports.plugin_alloc(args.length);
    new Uint8Array(memory.buffer).set(args, pointer);
    const packed = BigInt.asUintN(64, exported(hasParent, parent, pointer, args.length));
    if (!answered) return null;
    const at = Number(packed >> 32n);
    const size = Number(packed & 0xffffffffn);
    const answer = new Uint8Array(5 + size);
    answer.set(MARK, 0);
    answer[4] = RETURNED;
    answer.set(new Uint8Array(memory.buffer).subarray(at, at + size), 5);
    return answer;
}

function resolve(module, memory, shim, frames) {
    const resolved = { __proto__: null };
    for (const wanted of WebAssembly.Module.imports(module)) {
        const group = (resolved[wanted.module] ??= { __proto__: null });
        if (wanted.kind === "memory") {
            group[wanted.name] = memory();
        } else if (wanted.module === "wasi_snapshot_preview1") {
            group[wanted.name] = wasi[wanted.name] ?? stub(wanted.module, wanted.name);
        } else if (wanted.module === "wasi") {
            group[wanted.name] = threads[wanted.name];
        } else if (wanted.module === "be3_host" && wanted.name === "host_panic") {
            group[wanted.name] = (pointer, length) => {
                const bytes = new Uint8Array(memory().buffer).slice(pointer, pointer + length);
                throw new Error(new TextDecoder().decode(bytes));
            };
        } else if (
            wanted.module === "be3_host" &&
            (wanted.name === "host_send" || wanted.name === "host_receive")
        ) {
            group[wanted.name] = frames[wanted.name];
        } else if (frames.direct.some(([module, name]) => module === wanted.module && name === wanted.name)) {
            group[wanted.name] = frames.forward(`${wanted.module}#${wanted.name}`);
        } else if (wanted.module === "be3_host") {
            group[wanted.name] = forward(shim, wanted.module, wanted.name, wanted.name);
        } else {
            const exported = `${wanted.module}#${wanted.name}`;
            group[wanted.name] = forward(shim, wanted.module, wanted.name, exported);
        }
    }
    return resolved;
}

// WebAssembly.Module.imports does not report a memory's limits, and a memory
// has to be created with limits the module accepts, so the import section is
// read out of the bytes the worker already fetched. A module that imports no
// memory has null, and is bound to the memory it exports once it is
// instantiated.
function importedMemory(bytes) {
    const view = new DataView(bytes);
    let at = 8;
    const byte = () => view.getUint8(at++);
    const leb = () => {
        let value = 0;
        let shift = 0;
        for (;;) {
            const part = byte();
            value += (part & 0x7f) * 2 ** shift;
            if ((part & 0x80) === 0) {
                return value;
            }
            shift += 7;
        }
    };
    const name = () => {
        const length = leb();
        at += length;
    };
    while (at < view.byteLength) {
        const section = byte();
        const size = leb();
        const end = at + size;
        if (section !== 2) {
            at = end;
            continue;
        }
        const count = leb();
        for (let index = 0; index < count; index += 1) {
            name();
            name();
            const kind = byte();
            if (kind === 0) {
                leb();
            } else if (kind === 1) {
                byte();
                const flags = byte();
                leb();
                if (flags & 1) {
                    leb();
                }
            } else if (kind === 2) {
                const flags = byte();
                const initial = leb();
                const maximum = flags & 1 ? leb() : undefined;
                return { initial, maximum, shared: (flags & 2) !== 0 };
            } else if (kind === 3) {
                byte();
                byte();
            } else if (kind === 4) {
                byte();
                byte();
                leb();
            }
        }
        at = end;
    }
    return null;
}

// What a standalone plugin, or the root library under a linked one, runs
// before anything else: the WASI reactor's _initialize when it has one, else
// the constructors LLVM collected.
function initialize(exports) {
    if (typeof exports._initialize === "function") {
        exports._initialize();
    } else {
        exports.__wasm_call_ctors?.();
    }
}

async function fetched(url) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`${url} could not be fetched: ${response.status}`);
    }
    return response.arrayBuffer();
}

export async function boot(shimUrl, moduleUrl, wake, direct) {
    const shimModule = await import(shimUrl);
    const shim = await shimModule.default();
    await shimModule.start();
    const bytes = await fetched(moduleUrl);
    const plugin = await WebAssembly.compile(bytes);
    const identities = dylink.libraries(shimModule, plugin);
    const libraries = await Promise.all(
        identities.map((identity) => dylink.fetchLibrary(shimModule, identity, moduleUrl)),
    );
    const modules = [...libraries.map(({ module }) => module), plugin];
    const first = modules[0];
    const imported = importedMemory(libraries[0]?.bytes ?? bytes);
    if (libraries.length > 0 && !imported?.shared) {
        throw new Error("the root library does not import a shared memory");
    }
    let memory = imported === null ? null : new WebAssembly.Memory(imported);
    const current = () => memory;
    const bindMemory = () => {
        wasi.bindMemory(memory);
        shimModule.bind(memory);
        threads.bindThreads(first, memory);
    };
    if (memory !== null) bindMemory();
    threads.bindWake(wake);
    shimModule.bind_wake(wake);
    const frames = transport(current, direct);
    const runtime = new WebAssembly.Instance(first, resolve(first, current, shim, frames));
    let exports = runtime.exports;
    if (first !== plugin) {
        initialize(runtime.exports);
        const plan = dylink.plan(shimModule, modules, identities, runtime, memory);
        threads.bindLinked(modules, plan);
        const thread = {
            blocks: dylink.takeBlocks(memory, plan),
            rootBlock: runtime.exports.__tls_base.value >>> 0,
            main: true,
        };
        exports = dylink.link(modules, plan, runtime, thread, (module) =>
            resolve(module, current, shim, frames),
        ).exports;
    } else {
        if (memory === null && exports.memory instanceof WebAssembly.Memory) {
            memory = exports.memory;
            bindMemory();
        }
        initialize(exports);
    }
    exports.plugin_start();
    return {
        deliver(frame) {
            const item = new Uint8Array(frame);
            if (!(marked(item) && item[4] === CALL)) {
                frames.inbox.push(item);
                return;
            }
            if (frames.inbox.length > 0) exports.plugin_step();
            const answer = callExport(exports, memory, item);
            if (answer !== null) frames.post(answer);
        },
        collect: () => frames.collect(),
        show: (id, canvas, surface) => shimModule.show(id, canvas, surface),
        forget: (id) => shimModule.forget(id),
        layers: (canvas, width, height, pieces) => shimModule.layers(canvas, width, height, pieces),
        paint: () => shimModule.paint(),
        exportSurfaces: (surfaces) => shimModule.export_surfaces(Uint32Array.from(surfaces)),
        exported: () => shimModule.exported(),
        importImage: (image, bitmap) => shimModule.import_image(BigInt(image), bitmap),
        forgetImage: (image) => shimModule.forget_image(BigInt(image)),
        took: () => shimModule.took(),
        failure: () => shimModule.failure(),
        woken: () => shimModule.woken(),
        step: () => exports.plugin_step(),
        shutdown: () => exports.plugin_shutdown(),
    };
}
