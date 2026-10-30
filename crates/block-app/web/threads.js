// The wasi thread-spawn call the module makes.
//
// block-app and every plugin are built for wasm32-wasip1-threads, so a
// std::thread::spawn reaches wasi-libc's pthread implementation, which asks the
// host for a thread through the wasi-threads import "wasi"."thread-spawn". A
// browser is not a wasi-threads host, so a thread is a Worker that instantiates
// the very same module against the very same shared memory and calls the
// module's wasi_thread_start with the id and argument agreed here.
//
// A plugin linked against libraries is a list of modules, the root library
// first: the thread instantiates the root, and links the rest by the plan the
// plugin's own thread made (dylink.js), with thread-local blocks for the
// modules above the root that the spawning thread takes for it here and the
// thread gives back when it ends.
//
// Only the modules and their memory are shared. Apart from the wake, which the
// thread reports back here as a message, every other import the thread's
// instance is given is a stub that throws, because the JavaScript state that
// wasm-bindgen keeps beside a module — its object table, its cached views —
// belongs to the thread that made it. A thread is for computing, and reaching
// out of the module from one is a mistake worth hearing about.

import { spawnedThreadLocals, takeBlocks } from "./dylink.js";
import { filesChannel } from "./wasi.js";

/** @type {{module: WebAssembly.Module, memory: WebAssembly.Memory} | null} */
let context = null;

/** @type {{modules: WebAssembly.Module[], plan: object} | null} */
let linked = null;

/** @type {(() => void) | null} */
let onWake = null;

/**
 * Says what to do when a thread reports that a job it was started for landed.
 * The plugin worker steps the module again; the app has nothing to schedule
 * and leaves this unset.
 */
export function bindWake(wake) {
    onWake = wake;
}

// The id wasi-libc stores as the thread's own. Ids start above the main
// thread's so that no live thread is ever mistaken for another.
let nextId = 2;

/**
 * Hands the shim what a thread has to be started from: the compiled module and
 * the shared memory the running instance was given. Until this is called there
 * is nothing to start a thread from and spawning fails.
 */
export function bindThreads(module, memory) {
    context = { module, memory };
}

/**
 * Says that the module bound is the root library of these modules, linked by
 * this plan, so that a thread links them too.
 */
export function bindLinked(modules, plan) {
    linked = { modules, plan };
}

function threadSpawn(startArgument) {
    if (context === null) {
        console.error("a thread was spawned before bindThreads was called");
        return -1;
    }
    if (!(context.memory.buffer instanceof SharedArrayBuffer)) {
        console.error("a thread was spawned by a module whose memory is not shared");
        return -1;
    }
    const id = nextId++;
    const worker = new Worker(new URL("./thread.js", import.meta.url), { type: "module" });
    worker.onerror = (event) => {
        console.error(`thread ${id} failed`, event.message ?? event);
    };
    worker.onmessage = (event) => {
        if (event.data?.kind === "wake") {
            onWake?.();
        }
    };
    worker.postMessage({
        module: context.module,
        memory: context.memory,
        id,
        startArgument,
        modules: linked?.modules,
        plan: linked?.plan,
        blocks: linked && takeBlocks(context.memory, linked.plan),
        rootBlock: linked && spawnedThreadLocals(context.memory, startArgument),
        files: filesChannel(),
    });
    return id;
}

export { threadSpawn as "thread-spawn" };

// The worker the app's block stack runs in (be-worker.js), started with the
// first stack and kept for the page's life: what the stack opened there, such
// as its sockets, belongs to that worker's JavaScript.
/** @type {Worker | null} */
let stackWorker = null;

function stackWorkerStart(entry) {
    if (context === null) {
        console.error("the block stack was started before bindThreads was called");
        return;
    }
    if (stackWorker === null) {
        stackWorker = new Worker(new URL("./be-worker.js", import.meta.url), { type: "module" });
        stackWorker.onerror = (event) => {
            console.error("the block stack's worker failed", event.message ?? event);
        };
        stackWorker.postMessage({ module: context.module, memory: context.memory, files: filesChannel() });
    }
    stackWorker.postMessage({ entry: entry >>> 0 });
}

export { stackWorkerStart as "be-worker-start" };
