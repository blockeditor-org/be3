// Instantiating a plugin in the browser.
//
// A plugin is the same bindgen-free wasm that wasmtime runs on every other
// platform: its only imports are WASI and the be3 gpu abi, and, when it was
// linked against the plugin SDK, the SDK, which is fetched from beside it and
// linked to it here (dylink.js). In a browser the
// abi is answered by block-gpu-shim, a second module that does have bindgen
// and a real wgpu device in the worker, which draws what the plugin shows into
// the canvases the page transferred to the worker. The two modules have
// separate memories, so the shim is handed the plugin's and every import is
// bound straight to the shim's export of the same name, which reads and writes
// what a pointer points at in the plugin's memory itself.

import * as dylink from "./dylink.js";
import * as wasi from "./wasi.js";
import * as threads from "./threads.js";

function stub(module, name) {
    return () => {
        throw new Error(`a plugin called ${module}.${name}, which its host does not answer`);
    };
}

function forward(shim, name) {
    const call = shim[name];
    return typeof call === "function" ? call : stub("be3", name);
}

function resolve(module, memory, shim) {
    const resolved = { __proto__: null };
    for (const wanted of WebAssembly.Module.imports(module)) {
        const group = (resolved[wanted.module] ??= { __proto__: null });
        if (wanted.kind === "memory") {
            group[wanted.name] = memory;
        } else if (wanted.module === "wasi_snapshot_preview1") {
            group[wanted.name] = wasi[wanted.name] ?? stub(wanted.module, wanted.name);
        } else if (wanted.module === "wasi") {
            group[wanted.name] = threads[wanted.name];
        } else if (wanted.module === "be3_host" && wanted.name === "host_panic") {
            group[wanted.name] = (pointer, length) => {
                const bytes = new Uint8Array(memory.buffer).slice(pointer, pointer + length);
                throw new Error(new TextDecoder().decode(bytes));
            };
        } else if (wanted.module === "be3_gpu" || wanted.module === "be3_host") {
            group[wanted.name] = forward(shim, wanted.name);
        } else {
            group[wanted.name] = stub(wanted.module, wanted.name);
        }
    }
    return resolved;
}

// WebAssembly.Module.imports does not report a memory's limits, and a shared
// memory has to be created with limits the module accepts, so the import
// section is read out of the bytes the worker already fetched.
function memoryLimits(bytes) {
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
                return { initial, maximum };
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
    throw new Error("a plugin does not import the memory it runs in");
}

async function fetched(url) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`${url} could not be fetched: ${response.status}`);
    }
    return response.arrayBuffer();
}

export async function boot(shimUrl, moduleUrl, wake) {
    const shimModule = await import(shimUrl);
    const shim = await shimModule.default();
    await shimModule.start();
    const bytes = await fetched(moduleUrl);
    const plugin = await WebAssembly.compile(bytes);
    const identity = dylink.sdkIdentity(shimModule, plugin);
    const sdkBytes =
        identity === null ? null : await fetched(new URL(shimModule.sdk_file_name(identity), moduleUrl));
    if (sdkBytes !== null) {
        shimModule.check_sdk(new Uint8Array(sdkBytes), identity);
    }
    const first = sdkBytes === null ? plugin : await WebAssembly.compile(sdkBytes);
    const memory = new WebAssembly.Memory({ ...memoryLimits(sdkBytes ?? bytes), shared: true });
    wasi.bindMemory(memory);
    shimModule.bind(memory);
    threads.bindThreads(first, memory);
    threads.bindWake(wake);
    const runtime = new WebAssembly.Instance(first, resolve(first, memory, shim));
    runtime.exports.__wasm_call_ctors?.();
    let exports = runtime.exports;
    if (first !== plugin) {
        const layout = dylink.plan(shimModule, runtime, plugin, memory);
        const instance = dylink.instantiate(runtime, plugin, layout, resolve(plugin, memory, shim));
        dylink.relocate(instance, layout);
        instance.exports.plugin_initialize_tls(
            runtime.exports.__tls_size.value,
            runtime.exports.__tls_align.value,
        );
        dylink.placeThreadLocals(instance, memory, layout, runtime.exports.__tls_base.value);
        threads.bindPlugin(plugin, layout);
        instance.exports.__wasm_call_ctors?.();
        exports = instance.exports;
    } else {
        exports.plugin_initialize_tls(exports.__tls_size.value, exports.__tls_align.value);
    }
    exports.plugin_start();
    return {
        deliver: (frame) => shimModule.deliver(frame),
        collect: () => shimModule.collect(),
        show: (id, canvas, surface) => shimModule.show(id, canvas, surface),
        forget: (id) => shimModule.forget(id),
        paint: () => shimModule.paint(),
        took: () => shimModule.took(),
        failure: () => shimModule.failure(),
        woken: () => shimModule.woken(),
        step: () => exports.plugin_step(),
        shutdown: () => exports.plugin_shutdown(),
    };
}
