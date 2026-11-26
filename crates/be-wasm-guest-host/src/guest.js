// The worker every guest runs in in a browser: a type module, a pure guest the
// page compiled from bytes, or an app.
//
// be-wasm-guest-host embeds this file and starts it as a blob worker, so it needs
// no file served beside the app. The page steps the guest the way the native
// host does: it says "step" with the frames it has for the guest, and the worker
// steps once and answers with what came out. A guest that wakes between steps
// says so, so the page steps it again.
//
// On its own the worker answers be3_host and the few WASI calls a pure guest
// makes (pureWasi), and refuses any other import, which is all a type module
// has. An app's start names an
// extension, be-host's app.js, which instantiates the guest with its
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

// ChaCha20's keystream (RFC 7539) under the 32-byte `seed` as its key, with a
// zero nonce and the block counter from zero, drawn byte by byte: be-wasm-pure-wasi's
// stream, so a pure guest draws the same randomness here as natively.
export function keystream(seed) {
    const key = new Uint32Array(8);
    const seeded = new DataView(seed.buffer, seed.byteOffset, 32);
    for (let index = 0; index < 8; index++) key[index] = seeded.getUint32(index * 4, true);
    const initial = new Uint32Array(16);
    const state = new Uint32Array(16);
    const block = new Uint8Array(64);
    const words = new DataView(block.buffer);
    let counter = 0;
    let used = 64;
    const rotate = (value, by) => (value << by) | (value >>> (32 - by));
    function quarter(a, b, c, d) {
        state[a] += state[b];
        state[d] = rotate(state[d] ^ state[a], 16);
        state[c] += state[d];
        state[b] = rotate(state[b] ^ state[c], 12);
        state[a] += state[b];
        state[d] = rotate(state[d] ^ state[a], 8);
        state[c] += state[d];
        state[b] = rotate(state[b] ^ state[c], 7);
    }
    function next() {
        initial.set([0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
        initial.set(key, 4);
        initial[12] = counter;
        state.set(initial);
        for (let round = 0; round < 10; round++) {
            quarter(0, 4, 8, 12);
            quarter(1, 5, 9, 13);
            quarter(2, 6, 10, 14);
            quarter(3, 7, 11, 15);
            quarter(0, 5, 10, 15);
            quarter(1, 6, 11, 12);
            quarter(2, 7, 8, 13);
            quarter(3, 4, 9, 14);
        }
        for (let index = 0; index < 16; index++) {
            words.setUint32(index * 4, (state[index] + initial[index]) >>> 0, true);
        }
        counter = (counter + 1) >>> 0;
        used = 0;
    }
    return (bytes) => {
        for (let index = 0; index < bytes.length; index++) {
            if (used === 64) next();
            bytes[index] = block[used++];
        }
    };
}

// The WASI a pure guest is answered, as be-wasm-pure-wasi answers it natively:
// randomness from its seed, standard output and error to the console a line at
// a time, no arguments or environment, no clock, and exiting stops it.
export function pureWasi(guest, seed) {
    const SUCCESS = 0;
    const EBADF = 8;
    const ENOSYS = 52;
    if (seed !== undefined && seed.length !== 32) {
        throw new Error(`a pure guest's seed is 32 bytes, not ${seed.length}`);
    }
    const random = keystream(seed ?? new Uint8Array(32));
    const decoder = new TextDecoder();
    let line = "";
    const view = () => new DataView(guest.memory.buffer);
    const none = (count, size) => {
        view().setUint32(count >>> 0, 0, true);
        view().setUint32(size >>> 0, 0, true);
        return SUCCESS;
    };
    return {
        args_get: () => SUCCESS,
        args_sizes_get: none,
        environ_get: () => SUCCESS,
        environ_sizes_get: none,
        clock_res_get: () => ENOSYS,
        clock_time_get: () => ENOSYS,
        fd_write(fd, iovecs, count, written) {
            if (fd !== 1 && fd !== 2) return EBADF;
            const memory = view();
            let total = 0;
            for (let index = 0; index < count >>> 0; index++) {
                const at = (iovecs >>> 0) + index * 8;
                const pointer = memory.getUint32(at, true);
                const length = memory.getUint32(at + 4, true);
                line += decoder.decode(new Uint8Array(guest.memory.buffer, pointer, length), { stream: true });
                total += length;
                if (total >= guest.memory.buffer.byteLength) break;
            }
            const lines = line.split("\n");
            line = lines.pop();
            for (const complete of lines) console.error(complete);
            memory.setUint32(written >>> 0, total, true);
            return SUCCESS;
        },
        proc_exit(status) {
            throw new Error(`the guest exited with status ${status}`);
        },
        random_get(pointer, length) {
            random(new Uint8Array(guest.memory.buffer, pointer >>> 0, length >>> 0));
            return SUCCESS;
        },
    };
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

    // A start names the module's URL, and the extension that links it if
    // any, or hands over a pure guest's module the page compiled already
    // (be-wasm-sandbox-host's `compile`), which is instantiated as it is,
    // with the memory `data.memory` describes if it imports one. A pure guest
    // draws its randomness from `data.seed`, 32 bytes, or the zero seed.
    async function start(data) {
        const loading = data.extension ? import(new URL(data.extension, data.url).href) : null;
        let fetched = null;
        let module = data.module;
        if (!module) {
            const response = await fetch(data.url);
            if (!response.ok) throw new Error(`${data.url} could not be fetched: ${response.status}`);
            fetched = await response.arrayBuffer();
            module = await WebAssembly.compile(fetched);
        }
        const wake = () => post({ kind: "wake" });
        if (loading) {
            extension = await (await loading).boot({ url: data.url, bytes: fetched, module, guest, imports, initialize, wake });
        } else {
            // A pure guest from bytes imports its memory, which the page capped at
            // its spawner's limit and names here by its pages.
            const memory = data.memory ? new WebAssembly.Memory(data.memory) : undefined;
            const wasi = pureWasi(guest, data.seed);
            const answer = (wanted) => {
                if (wanted.kind === "memory") return memory;
                if (wanted.module === "wasi_snapshot_preview1" && Object.hasOwn(wasi, wanted.name)) return wasi[wanted.name];
                return undefined;
            };
            const instance = await WebAssembly.instantiate(module, imports(module, answer));
            guest.exports = instance.exports;
            guest.memory = memory ?? guest.exports.memory;
            initialize(guest.exports);
        }
        guest.exports.app_start();
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
                    guest.exports.app_step();
                    report("stepped", performance.now() - started);
                } else if (data.kind === "shutdown") {
                    if (guest.exports !== null) guest.exports.app_shutdown();
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
