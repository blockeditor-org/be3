// The host's worker, which keeps the host's files (crates/be-host/src/host_files.rs) and runs
// its block stacks (crates/be-host-stack/src/web.rs).
//
// Unlike a thread (thread.js), this worker has the module's JavaScript glue
// beside it: it instantiates the module again against the page's shared memory
// through the wasm-bindgen glue, which gives this instance a stack and
// thread-local storage of its own, and then runs each work the page starts, a
// named entry and the bytes it starts from (be-host's be_worker_run), as a
// future on this worker's own event loop. Its sockets, timers and fetches
// are its own, and its file calls wait for files.js rather than spin.
//
// A worker sees no import map, so the glue is loaded from a copy whose bare
// specifiers are rewritten to the shims' addresses.

import * as wasi from "./wasi.js";
import * as environment from "./env.js";
import * as threads from "./threads.js";

// Larger than wasm-bindgen's default of 1 MiB: the stack runs SQLite and
// deep futures, as on a native thread.
const STACK_SIZE = 8 << 20;

async function glue() {
    const base = import.meta.url;
    let source = await (await fetch(new URL("./be_host.js", base))).text();
    for (const [bare, file] of [
        ["wasi_snapshot_preview1", "./wasi.js"],
        ["wasi", "./threads.js"],
        ["env", "./env.js"],
    ]) {
        source = source.replaceAll(`from "${bare}"`, `from "${new URL(file, base)}"`);
    }
    const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
    try {
        return await import(url);
    } finally {
        URL.revokeObjectURL(url);
    }
}

let started = null;

async function start({ module, memory, files, letters }) {
    wasi.bindMemory(memory);
    environment.bindMemory(memory);
    threads.bindThreads(module, memory);
    threads.bindLetters(letters);
    wasi.bindFiles(files);
    const loaded = await glue();
    loaded.initSync({ module, memory, thread_stack_size: STACK_SIZE });
    return loaded;
}

self.onmessage = async (event) => {
    if (event.data.module) {
        started = start(event.data);
        return;
    }
    const loaded = await started;
    try {
        await loaded.be_worker_run(event.data.entry, event.data.port, event.data.bytes);
    } catch (error) {
        console.error(`the host's worker's ${event.data.entry} stopped`, error);
    }
};
