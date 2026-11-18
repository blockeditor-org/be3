// The host's worker, which keeps the host's files (crates/be-host/src/host_files.rs) and runs
// its block stacks (crates/be-host-stack/src/web.rs).
//
// Unlike a thread (thread.js), this worker has the module's JavaScript glue
// beside it and a memory of its own: it instantiates the module through the
// wasm-bindgen glue (start, below), and then runs each work the page starts, a
// named entry, the bytes it starts from and the port it talks to the page
// through (be-host's be_worker_run), as a future on this worker's own event
// loop. It shares no memory with the page: everything between them is a
// message. Its sockets, timers and fetches
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

// The worker's memory is its own, not the page's (decisions/the_web_page_never_waits.md). A first
// instance makes it, and is its main thread, which runs nothing after its start function; a
// second on the same memory runs the work, on a stack of STACK_SIZE, as a thread's, where
// wasm-bindgen's main thread has the linker's 1 MiB. A copy of the glue holds one instance, so
// each is loaded on its own.
async function start({ module, files, letters }) {
    const memory = (await glue()).initSync({ module }).memory;
    wasi.bindMemory(memory);
    environment.bindMemory(memory);
    threads.bindThreads(module, memory);
    threads.bindLetters(letters);
    wasi.bindFiles(files);
    // files.js answers this worker's file calls, in this memory (threads.js hands it on).
    self.postMessage({ filesMemory: memory });
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
        // Not an error event, so the page is told outright: its ports hear the worker went.
        self.postMessage({ failed: true });
    }
};
