// The wasi_snapshot_preview1 calls the module makes.
//
// block-app is built for wasm32-wasip1-threads, so Rust's standard library and
// the C in FreeType and HarfBuzz both reach the host through WASI. wasm-bindgen
// emits those as imports from the bare specifier "wasi_snapshot_preview1",
// which index.html points at this file with an import map.
//
// A browser is not a WASI host: there is no environment and no process to
// exit. Only the calls the module actually makes are implemented, and the rest
// report that they are unsupported rather than pretending to succeed, so a
// wrong assumption surfaces as a clear error instead of silently corrupting
// state.
//
// The file system is the app's only: once the page binds files.js's channel
// (bindFiles), every call on a descriptor other than the standard streams is
// forwarded to that worker, which answers it from the origin private file
// system, and the caller waits for the answer. A worker waits with
// Atomics.wait; the page, which may not block, spins. A plugin binds no
// channel and has no file system at all.
//
// The memory is shared, so it is a SharedArrayBuffer. TextDecoder and
// getRandomValues both refuse to look at one, and every call that reads or
// writes bytes copies through an unshared buffer for that reason.

/** @type {WebAssembly.Memory | null} */
let memory = null;

// The calls files.js answers, by their index in this list.
export const FILE_CALLS = [
    "fd_prestat_get",
    "fd_prestat_dir_name",
    "path_open",
    "fd_close",
    "fd_read",
    "fd_pread",
    "fd_write",
    "fd_pwrite",
    "fd_seek",
    "fd_tell",
    "fd_sync",
    "fd_datasync",
    "fd_fdstat_get",
    "fd_fdstat_set_flags",
    "fd_filestat_get",
    "fd_filestat_set_size",
    "fd_filestat_set_times",
    "fd_advise",
    "fd_allocate",
    "fd_readdir",
    "path_filestat_get",
    "path_filestat_set_times",
    "path_create_directory",
    "path_remove_directory",
    "path_unlink_file",
    "path_rename",
    "path_readlink",
    "path_symlink",
    "path_link",
];

// The channel's control words: a lock between callers, the state of the one
// call in flight, which call it is, and its errno. Its arguments are a
// BigInt64Array beside it.
export const CONTROL = { LOCK: 0, STATE: 1, CALL: 2, RESULT: 3, IDLE: 0, ASKED: 1, ANSWERED: 2, WORDS: 4, ARGUMENTS: 10 };

/** @type {{control: Int32Array, args: BigInt64Array} | null} */
let files = null;

const canBlock = typeof WorkerGlobalScope !== "undefined" && self instanceof WorkerGlobalScope;

/**
 * Hands the shim the channel to files.js: two SharedArrayBuffers, the
 * control words and the arguments. A thread is handed the same pair.
 */
export function bindFiles(channel) {
    files = channel && {
        channel,
        control: new Int32Array(channel.control),
        args: new BigInt64Array(channel.args),
    };
}

/** The channel bindFiles was given, to hand on to a thread. */
export function filesChannel() {
    return files?.channel ?? null;
}

function waitWhile(control, index, value) {
    while (Atomics.load(control, index) === value) {
        if (canBlock) {
            Atomics.wait(control, index, value, 1000);
        }
    }
}

function forward(name, values) {
    const { control, args } = files;
    while (Atomics.compareExchange(control, CONTROL.LOCK, 0, 1) !== 0) {
        waitWhile(control, CONTROL.LOCK, 1);
    }
    try {
        values.forEach((value, index) => {
            // A 32-bit argument is a pointer, a length, a descriptor or
            // flags, all unsigned; a 64-bit one arrives as a BigInt.
            args[index] = typeof value === "bigint" ? value : BigInt(value >>> 0);
        });
        control[CONTROL.CALL] = FILE_CALLS.indexOf(name);
        Atomics.store(control, CONTROL.STATE, CONTROL.ASKED);
        Atomics.notify(control, CONTROL.STATE);
        waitWhile(control, CONTROL.STATE, CONTROL.ASKED);
        const result = control[CONTROL.RESULT];
        Atomics.store(control, CONTROL.STATE, CONTROL.IDLE);
        return result;
    } finally {
        Atomics.store(control, CONTROL.LOCK, 0);
        Atomics.notify(control, CONTROL.LOCK, 1);
    }
}

function isStream(fd) {
    return fd === 0 || fd === STDOUT || fd === STDERR;
}

/**
 * Hands the shim the module's memory. The wasm-bindgen glue runs the module's
 * start function during initialisation, before it returns the exports, so this
 * cannot be called any earlier than immediately after `init()` resolves.
 */
export function bindMemory(exported) {
    memory = exported;
}

// WASI errno values.
const SUCCESS = 0;
const EBADF = 8;
const EINVAL = 28;
const ENOENT = 44;
const ENOTSUP = 58;

// WASI filetype values.
const FILETYPE_CHARACTER_DEVICE = 2;

const STDOUT = 1;
const STDERR = 2;

function view() {
    if (memory === null) {
        throw new Error(
            "a WASI call was made before the module's memory was bound; " +
                "bindMemory must be called as soon as init() resolves",
        );
    }
    return new DataView(memory.buffer);
}

function bytes(pointer, length) {
    if (memory === null) {
        throw new Error("a WASI call was made before the module's memory was bound");
    }
    return new Uint8Array(memory.buffer, pointer, length);
}

function copied(pointer, length) {
    return bytes(pointer, length).slice();
}

// Text written to a stream is buffered until a newline, so a line logged in
// pieces arrives in the console as one message.
const pending = new Map();
const decoder = new TextDecoder();

function writeStream(fd, text) {
    const buffered = (pending.get(fd) ?? "") + text;
    const lines = buffered.split("\n");
    pending.set(fd, lines.pop() ?? "");
    for (const line of lines) {
        if (fd === STDERR) {
            console.error(line);
        } else {
            console.log(line);
        }
    }
}

function stream_fd_write(fd, iovsPointer, iovsLength, writtenPointer) {
    if (fd !== STDOUT && fd !== STDERR) {
        return EBADF;
    }
    const data = view();
    let written = 0;
    let text = "";
    for (let index = 0; index < iovsLength; index++) {
        const entry = iovsPointer + index * 8;
        const bufferPointer = data.getUint32(entry, true);
        const bufferLength = data.getUint32(entry + 4, true);
        text += decoder.decode(copied(bufferPointer, bufferLength));
        written += bufferLength;
    }
    writeStream(fd, text);
    view().setUint32(writtenPointer, written, true);
    return SUCCESS;
}

function stream_fd_read(fd, iovsPointer, iovsLength, readPointer) {
    // Nothing can be read: report end of file rather than an error, which is
    // what a closed stdin looks like.
    view().setUint32(readPointer, 0, true);
    return SUCCESS;
}

function stream_fd_close() {
    return SUCCESS;
}

function stream_fd_seek() {
    // The standard streams are not seekable.
    return ENOTSUP;
}

function stream_fd_fdstat_get(fd, resultPointer) {
    if (fd !== STDOUT && fd !== STDERR && fd !== 0) {
        return EBADF;
    }
    // fdstat is { filetype: u8, flags: u16 at offset 2, rights: u64 at 8,
    // rights_inheriting: u64 at 16 }. Reporting a character device keeps the
    // standard library from treating stdout as a seekable file.
    const data = view();
    data.setUint8(resultPointer, FILETYPE_CHARACTER_DEVICE);
    data.setUint16(resultPointer + 2, 0, true);
    data.setBigUint64(resultPointer + 8, 0n, true);
    data.setBigUint64(resultPointer + 16, 0n, true);
    return SUCCESS;
}

function stream_fd_fdstat_set_flags() {
    return SUCCESS;
}

function none_fd_filestat_get() {
    return EBADF;
}

function none_fd_prestat_get() {
    // No preopened directories. Reporting EBADF ends the standard library's
    // scan for them, which is what leaves the module with no filesystem.
    return EBADF;
}

function none_fd_prestat_dir_name() {
    return EBADF;
}

function none_path_open() {
    // There is no filesystem, so every path is missing. The app only reaches
    // here through code paths that already tolerate a failed open, such as the
    // text editor's search for system fonts, which the web build answers with
    // fonts held in the binary instead.
    return ENOENT;
}

// The rest of the filesystem calls, which C linked into a plugin (PDFium's
// search for system fonts, which the plugin turns off) imports whether or not
// it reaches them. With no directories there is nothing to list, stat or remove.

function none_fd_readdir() {
    return EBADF;
}

function none_path_filestat_get() {
    return ENOENT;
}

function none_path_remove_directory() {
    return ENOENT;
}

function none_path_unlink_file() {
    return ENOENT;
}

export function environ_sizes_get(countPointer, sizePointer) {
    const data = view();
    data.setUint32(countPointer, 0, true);
    data.setUint32(sizePointer, 0, true);
    return SUCCESS;
}

export function environ_get() {
    // Nothing to write: the environment is empty.
    return SUCCESS;
}

export function clock_time_get(clockId, precision, resultPointer) {
    // Clock ids: 0 realtime, 1 monotonic. Both are reported in nanoseconds.
    const milliseconds = clockId === 0 ? Date.now() : performance.now();
    view().setBigUint64(resultPointer, BigInt(Math.round(milliseconds * 1e6)), true);
    return SUCCESS;
}

export function random_get(pointer, length) {
    const filled = new Uint8Array(length);
    crypto.getRandomValues(filled);
    bytes(pointer, length).set(filled);
    return SUCCESS;
}

export function sched_yield() {
    // A thread cannot hand the rest of its slice to another one from inside
    // WebAssembly, and the caller only needs the call to not fail.
    return SUCCESS;
}

export function proc_exit(code) {
    throw new Error(`Block exited with status ${code}`);
}


// A file call: the standard streams are answered here, anything else by
// files.js when the page bound it, and as if there were no files when not.
function file(name, stream, none) {
    return (...values) => {
        if (stream && isStream(values[0])) {
            return stream(...values);
        }
        if (files !== null) {
            return forward(name, values);
        }
        if (none) {
            return none(...values);
        }
        return name.startsWith("fd_") ? EBADF : ENOENT;
    };
}

export const fd_prestat_get = file("fd_prestat_get", null, none_fd_prestat_get);
export const fd_prestat_dir_name = file("fd_prestat_dir_name", null, none_fd_prestat_dir_name);
export const path_open = file("path_open", null, none_path_open);
export const fd_close = file("fd_close", stream_fd_close, null);
export const fd_read = file("fd_read", stream_fd_read, null);
export const fd_pread = file("fd_pread", null, null);
export const fd_write = file("fd_write", stream_fd_write, null);
export const fd_pwrite = file("fd_pwrite", null, null);
export const fd_seek = file("fd_seek", stream_fd_seek, null);
export const fd_tell = file("fd_tell", null, null);
export const fd_sync = file("fd_sync", null, null);
export const fd_datasync = file("fd_datasync", null, null);
export const fd_fdstat_get = file("fd_fdstat_get", stream_fd_fdstat_get, null);
export const fd_fdstat_set_flags = file("fd_fdstat_set_flags", stream_fd_fdstat_set_flags, null);
export const fd_filestat_get = file("fd_filestat_get", null, none_fd_filestat_get);
export const fd_filestat_set_size = file("fd_filestat_set_size", null, null);
export const fd_filestat_set_times = file("fd_filestat_set_times", null, null);
export const fd_advise = file("fd_advise", null, null);
export const fd_allocate = file("fd_allocate", null, null);
export const fd_readdir = file("fd_readdir", null, none_fd_readdir);
export const path_filestat_get = file("path_filestat_get", null, none_path_filestat_get);
export const path_filestat_set_times = file("path_filestat_set_times", null, null);
export const path_create_directory = file("path_create_directory", null, null);
export const path_remove_directory = file("path_remove_directory", null, none_path_remove_directory);
export const path_unlink_file = file("path_unlink_file", null, none_path_unlink_file);
export const path_rename = file("path_rename", null, null);
export const path_readlink = file("path_readlink", null, null);
export const path_symlink = file("path_symlink", null, null);
export const path_link = file("path_link", null, null);

export function poll_oneoff(subscriptions, events, count, written) {
    // Only a clock is waited on: the longest wait asked for is slept in a
    // worker, and every subscription is then reported as having fired. The
    // page cannot sleep and returns at once.
    const data = view();
    let longest = 0n;
    for (let index = 0; index < count; index++) {
        const at = subscriptions + index * 48;
        if (data.getUint8(at + 8) === 0) {
            const timeout = data.getBigUint64(at + 24, true);
            longest = timeout > longest ? timeout : longest;
        }
    }
    if (canBlock && longest > 0n) {
        Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, Number(longest / 1000000n));
    }
    for (let index = 0; index < count; index++) {
        const from = subscriptions + index * 48;
        const at = events + index * 32;
        data.setBigUint64(at, data.getBigUint64(from, true), true);
        data.setUint16(at + 8, SUCCESS, true);
        data.setUint8(at + 10, data.getUint8(from + 8));
        data.setBigUint64(at + 16, 0n, true);
        data.setUint16(at + 24, 0, true);
    }
    data.setUint32(written, count, true);
    return SUCCESS;
}
