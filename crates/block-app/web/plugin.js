// Instantiating a plugin in the browser: the extension block-guest-host's
// worker (src/guest.js) loads for a plugin, which answers the imports that
// worker does not.
//
// A plugin is the same bindgen-free wasm that wasmtime runs on every other
// platform: its only imports are be3_host, its world's and the be3 gpu abi,
// WASI and wasi-threads if it uses them, and, when it was linked against
// libraries, the libraries, which are fetched from beside it and linked to it
// here (dylink.js). A standalone plugin is instantiated as it is, with the memory it
// imports, shared or not, or the one it exports (guides/plugin_libraries.md, "The
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
// The worker answers be3_host's frames, wake and panic and the plugin's direct
// calls itself, as it does for a type module, and hands the rest to `answer`.

import * as dylink from "./dylink.js";
import * as wasi from "./wasi.js";
import * as threads from "./threads.js";

function stub(module, name) {
    return () => {
        throw new Error(`a plugin called ${module}.${name}, which its host does not answer`);
    };
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

function answer(wanted, memory, shim) {
    if (wanted.kind === "memory") return memory();
    if (wanted.module === "wasi_snapshot_preview1") return wasi[wanted.name] ?? stub(wanted.module, wanted.name);
    if (wanted.module === "wasi") return threads[wanted.name];
    const exported = wanted.module === "be3_host" ? wanted.name : `${wanted.module}#${wanted.name}`;
    const call = shim[exported];
    return typeof call === "function" ? call : stub(wanted.module, wanted.name);
}

export async function boot({ url, bytes, module: plugin, guest, imports, initialize, wake }) {
    const shimModule = await import(new URL("./block_gpu_shim.js", import.meta.url).href);
    const shim = await shimModule.default();
    await shimModule.start();
    const identities = dylink.libraries(shimModule, plugin);
    const libraries = await Promise.all(
        identities.map((identity) => dylink.fetchLibrary(shimModule, identity, url)),
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
        guest.memory = memory;
        wasi.bindMemory(memory);
        shimModule.bind(memory);
        threads.bindThreads(first, memory);
    };
    if (memory !== null) bindMemory();
    threads.bindWake(wake);
    shimModule.bind_wake(wake);
    const resolve = (module) => imports(module, (wanted) => answer(wanted, current, shim));
    const runtime = new WebAssembly.Instance(first, resolve(first));
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
        exports = dylink.link(modules, plan, runtime, thread, resolve).exports;
    } else {
        if (memory === null && exports.memory instanceof WebAssembly.Memory) {
            memory = exports.memory;
            bindMemory();
        }
        initialize(exports);
    }
    guest.exports = exports;
    return {
        receive(data) {
            if (data.kind === "show") {
                shimModule.show(data.id, data.canvas, data.surface);
                shimModule.paint();
            } else if (data.kind === "layers") {
                shimModule.layers(data.canvas ?? null, data.width, data.height, data.pieces);
                shimModule.paint();
            } else if (data.kind === "forget") {
                shimModule.forget(data.id);
            } else if (data.kind === "export") {
                shimModule.export_surfaces(Uint32Array.from(data.surfaces));
            } else if (data.kind === "import") {
                shimModule.import_image(BigInt(data.image), data.bitmap);
            } else if (data.kind === "unimport") {
                shimModule.forget_image(BigInt(data.image));
            }
        },
        report() {
            const painted = shimModule.paint();
            const [gpu, submit] = shimModule.took();
            const pictures = shimModule.exported();
            return {
                fields: { painted, pictures, gpu, submit },
                transfer: pictures.map((picture) => picture.bitmap),
                failure: shimModule.failure(),
            };
        },
    };
}
