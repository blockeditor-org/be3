// The worker every guest runs in in a browser: a type module, or a plugin.
//
// block-guest-host embeds this file and starts it as a blob worker, so it needs
// no file served beside the app. The page steps the guest the way the native
// host does: it says "step" with the frames it has for the guest, and the worker
// steps once and answers with what came out. A guest that wakes between steps
// says so, so the page steps it again.
//
// On its own the worker answers be3_host and refuses any other import, which is
// all a type module has. A plugin's start names an
// extension, block-app's plugin.js, which instantiates the guest with its
// libraries, WASI, threads and the gpu, answers what the worker does not,
// handles the page's other messages and adds what it painted to each report.

// What a standalone guest, or the root library under a linked one, runs before
// anything else: the WASI reactor's _initialize when it has one, else the
// constructors LLVM collected.
function initialize(exports) {
    if (typeof exports._initialize === "function") {
        exports._initialize();
    } else {
        exports.__wasm_call_ctors?.();
    }
}

export function runtime(post) {
    const guest = { exports: null, memory: null };
    let extension = null;
    let held = [];
    let running = false;
    let inbox = [];
    let outbox = [];
    let woken = false;

    const bytes = () => new Uint8Array(guest.memory.buffer);

    const host = {
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
        host_wake() {
            woken = true;
        },
        host_panic(pointer, length) {
            throw new Error(new TextDecoder().decode(bytes().slice(pointer, pointer + length)));
        },
    };

    // The imports of one module, with `answer` asked for any the worker does
    // not answer itself; an import nobody answers is refused.
    function imports(module, answer = () => undefined) {
        const resolved = { __proto__: null };
        for (const wanted of WebAssembly.Module.imports(module)) {
            let answered;
            if (wanted.module === "be3_host" && Object.hasOwn(host, wanted.name)) {
                answered = host[wanted.name];
            } else {
                answered = answer(wanted);
            }
            if (answered === undefined) {
                throw new Error(`the guest imports ${wanted.module}.${wanted.name}, which its host does not answer`);
            }
            (resolved[wanted.module] ??= { __proto__: null })[wanted.name] = answered;
        }
        return resolved;
    }

    function fail(error) {
        guest.exports = null;
        running = false;
        post({ kind: "error", message: String((error && error.message) || error) });
    }

    function report(kind, step) {
        const extra = extension?.report() ?? {};
        const frames = outbox.map((frame) => frame.buffer);
        outbox = [];
        const wasWoken = woken;
        woken = false;
        post({ ...extra.fields, kind, frames, woken: wasWoken, step }, [...frames, ...(extra.transfer ?? [])]);
        if (extra.failure) fail(extra.failure);
    }

    async function start(data) {
        const loading = data.extension ? import(new URL(data.extension, data.url).href) : null;
        const response = await fetch(data.url);
        if (!response.ok) throw new Error(`${data.url} could not be fetched: ${response.status}`);
        const fetched = await response.arrayBuffer();
        const module = await WebAssembly.compile(fetched);
        const wake = () => post({ kind: "wake" });
        if (loading) {
            extension = await (await loading).boot({ url: data.url, bytes: fetched, module, guest, imports, initialize, wake });
        } else {
            const instance = await WebAssembly.instantiate(module, imports(module));
            guest.exports = instance.exports;
            guest.memory = guest.exports.memory;
            initialize(guest.exports);
        }
        guest.exports.plugin_start();
        for (const message of held) extension?.receive(message);
        held = [];
        running = true;
        report("ready", 0);
    }

    return {
        async receive(data) {
            try {
                if (data.kind === "start") {
                    await start(data);
                } else if (data.kind === "step") {
                    if (!running) return;
                    const started = performance.now();
                    for (const frame of data.frames) inbox.push(new Uint8Array(frame));
                    guest.exports.plugin_step();
                    report("stepped", performance.now() - started);
                } else if (data.kind === "shutdown") {
                    if (guest.exports !== null) guest.exports.plugin_shutdown();
                    self.close();
                } else if (!running) {
                    held.push(data);
                } else {
                    extension?.receive(data);
                }
            } catch (error) {
                fail(error);
            }
        },
    };
}

export function serve() {
    const guest = runtime((message, transfer) => self.postMessage(message, transfer));
    self.onmessage = (event) => guest.receive(event.data);
}
