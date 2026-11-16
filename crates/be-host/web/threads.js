// The wasi thread-spawn call the module makes.
//
// be-host and every app are built for wasm32-wasip1-threads, so a
// std::thread::spawn reaches wasi-libc's pthread implementation, which asks the
// host for a thread through the be-wasm-threads import "wasi"."thread-spawn". A
// browser is not a be-wasm-threads host, so a thread is a Worker that instantiates
// the very same module against the very same shared memory and calls the
// module's wasi_thread_start with the id and argument agreed here.
//
// An app linked against libraries is a list of modules, the root library
// first: the thread instantiates the root, and links the rest by the plan the
// app's own thread made (dylink.js), with thread-local blocks for the
// modules above the root that the spawning thread takes for it here and the
// thread gives back when it ends.
//
// Only the modules and their memory are shared. Apart from the wake, which the
// thread reports back here as a message, and a letter, which it posts to the
// page (below), every other import the thread's
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
 * The app worker steps the module again; the host has nothing to schedule
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
        letters: lettersChannel(),
    });
    return id;
}

export { threadSpawn as "thread-spawn" };

// The host's worker (be-worker.js), which keeps the host's files and runs its
// block stacks, started with the first work handed to it and kept for the
// page's life: what a stack opened there, such as its sockets, belongs to that
// worker's JavaScript. Each work is a named entry, the bytes it starts from,
// copied out of the page's memory here, and a port the page and the entry talk
// through after, whose other end this answers (be_host_stack::start_in_worker,
// which calls this as the global beWorkerStart).
/** @type {Worker | null} */
let stackWorker = null;

/** @type {{worker: Worker, channel: object} | null} */
let hostFiles = null;

/**
 * Hands the shim files.js's worker and the channel to it, which the page binds no call to:
 * the host's worker makes the host's file calls, in a memory of its own.
 */
export function bindHostFiles(files) {
    hostFiles = files;
}

// The page's ends of the ports it started work with, held weakly: a worker that fails (a trap)
// drops none of its own ends, so each of these is told the worker went as an empty message
// would tell it (be_host_stack::Port).
const pagePorts = new Set();

function workerFailed() {
    for (const held of pagePorts) {
        held.deref()?.dispatchEvent(new MessageEvent("message", { data: new Uint8Array(0) }));
    }
    pagePorts.clear();
}

function stackWorkerStart(entry, bytes) {
    const channel = new MessageChannel();
    if (context === null) {
        console.error("the block stack was started before bindThreads was called");
        return channel.port1;
    }
    if (stackWorker === null) {
        stackWorker = new Worker(new URL("./be-worker.js", import.meta.url), { type: "module" });
        stackWorker.onerror = (event) => {
            console.error("the block stack's worker failed", event.message ?? event);
            workerFailed();
        };
        // Its file calls go to files.js, in the memory it makes, which it names once it has.
        stackWorker.onmessage = (event) => {
            if (event.data?.filesMemory) {
                hostFiles?.worker.postMessage({ memory: event.data.filesMemory });
            }
            // An entry failed without an error event (be-worker.js caught it).
            if (event.data?.failed) {
                workerFailed();
            }
        };
        stackWorker.postMessage({
            module: context.module,
            files: hostFiles?.channel ?? null,
            letters: lettersChannel(),
        });
    }
    stackWorker.postMessage({ entry, port: channel.port2, bytes: bytes.slice() }, [channel.port2]);
    for (const held of pagePorts) {
        if (!held.deref()) pagePorts.delete(held);
    }
    pagePorts.add(new WeakRef(channel.port1));
    return channel.port1;
}

globalThis.beWorkerStart = stackWorkerStart;

// Letters (be-host-base's host::Across::send) a worker posts to a mailbox the
// page made. The page shares no lock with a worker for them
// (plans/web_page_never_waits.md): the worker posts the letter's bytes, copied
// out of its memory, on a channel only this page's workers know the name of,
// and the page hands them to the module, which copies them in, puts the letter
// in the page's own list and wakes the host. A channel delivers what one
// worker posts in the order it posted it.
/** @type {{name: string, channel: BroadcastChannel, page: boolean} | null} */
let letters = null;

/**
 * Makes the page the one that takes letters, handing each to `arrived` (the
 * module's be_letter_arrived). The page calls it before it starts a worker.
 */
export function receiveLetters(arrived) {
    const name = `be-letters-${crypto.randomUUID()}`;
    const channel = new BroadcastChannel(name);
    channel.onmessage = (event) => arrived(event.data.mailbox, event.data.bytes);
    letters = { name, channel, page: true };
}

/** The name receiveLetters chose, to hand on to a worker. */
function lettersChannel() {
    return letters?.name ?? null;
}

/** In a worker: posts letters on the channel the page named. */
export function bindLetters(name) {
    letters = name ? { name, channel: new BroadcastChannel(name), page: false } : null;
}

// A letter no page can take is reported and dropped: throwing into the poster's wasm would
// skip its Rust destructors, and could leave a lock it holds locked for good.
function letterPost(mailbox, pointer, length) {
    if (letters === null || letters.page) {
        console.error(
            letters === null
                ? "a letter was posted with no page to take it"
                : "the page posted a letter to another thread's mailbox, which only the page makes",
        );
        return;
    }
    const bytes = new Uint8Array(context.memory.buffer).slice(pointer >>> 0, (pointer >>> 0) + (length >>> 0));
    letters.channel.postMessage({ mailbox, bytes });
}

export { letterPost as "be-letter-post" };
