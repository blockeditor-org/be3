// The file system the app's WASI calls reach in a browser.
//
// block-app keeps its state in files on every platform: the object store's
// directory, the refs database and the app state's database. In a browser
// those live in the origin private file system (OPFS), whose only synchronous
// way in, a FileSystemSyncAccessHandle, has to be opened asynchronously and
// only from a worker. A WASI call is synchronous, and the app makes them from
// the page and from its threads alike, so this worker owns every handle and
// answers each call: wasi.js forwards a call's name and arguments through a
// shared buffer and waits, and this worker reads and writes what the call
// points at in the module's shared memory itself.
//
// The page starts it before the module (block.js), hands it the module's
// memory once there is one, and only then tells wasi.js to forward, so the
// first path the module opens already finds the one preopened directory, "/".
// Nothing else in the origin opens these files: the page holds a Web Lock for
// as long as it runs, and a second tab waits for it, so handles stay open
// between calls rather than being reopened for every one.
//
// Where the origin private file system is unavailable, as in some private
// windows, the same calls are answered from memory, and nothing survives a
// reload.

import { FILE_CALLS, CONTROL } from "./wasi.js";

// WASI errno values.
const SUCCESS = 0;
const EBADF = 8;
const EBUSY = 10;
const EEXIST = 20;
const EINVAL = 28;
const EIO = 29;
const EISDIR = 31;
const ENOENT = 44;
const ENOSPC = 51;
const ENOTDIR = 54;
const ENOTEMPTY = 55;
const ENOTSUP = 58;

// WASI filetype values.
const FILETYPE_DIRECTORY = 3;
const FILETYPE_REGULAR_FILE = 4;

// path_open's oflags.
const OFLAGS_CREAT = 1;
const OFLAGS_DIRECTORY = 2;
const OFLAGS_EXCL = 4;
const OFLAGS_TRUNC = 8;

// fdflags.
const FDFLAGS_APPEND = 1;

const PREOPEN = 3;

// Sync access handles kept open with no descriptor on them. Each holds the
// file open, so the least recently used are closed beyond this many.
const IDLE_HANDLES = 256;

/** @type {WebAssembly.Memory | null} */
let memory = null;
let memoryBound;
const memoryReady = new Promise((resolve) => (memoryBound = resolve));

class Failure extends Error {
    constructor(errno) {
        super(`errno ${errno}`);
        this.errno = errno;
    }
}

function errnoOf(error) {
    if (error instanceof Failure) {
        return error.errno;
    }
    switch (error?.name) {
        case "NotFoundError":
            return ENOENT;
        case "TypeMismatchError":
            return ENOTDIR;
        case "InvalidModificationError":
            return ENOTEMPTY;
        case "NoModificationAllowedError":
            return EBUSY;
        case "QuotaExceededError":
            return ENOSPC;
        default:
            console.error("a file call failed", error);
            return EIO;
    }
}

// The same calls in memory, shaped like the parts of the origin private file
// system this worker uses.
class MemoryFile {
    kind = "file";
    bytes = new Uint8Array(0);
    size = 0;

    async createSyncAccessHandle() {
        const file = this;
        return {
            read(target, { at }) {
                const count = Math.max(0, Math.min(target.length, file.size - at));
                target.set(file.bytes.subarray(at, at + count));
                return count;
            },
            write(source, { at }) {
                file.reserve(at + source.length);
                file.bytes.set(source, at);
                file.size = Math.max(file.size, at + source.length);
                return source.length;
            },
            getSize: () => file.size,
            truncate(size) {
                file.reserve(size);
                file.bytes.fill(0, Math.min(size, file.size), size);
                file.size = size;
            },
            flush() {},
            close() {},
        };
    }

    reserve(size) {
        if (size <= this.bytes.length) {
            return;
        }
        const grown = new Uint8Array(Math.max(size, this.bytes.length * 2));
        grown.set(this.bytes.subarray(0, this.size));
        this.bytes = grown;
    }
}

class MemoryDirectory {
    kind = "directory";
    children = new Map();

    child(name, kind, create, made) {
        const found = this.children.get(name);
        if (found) {
            if (found.kind !== kind) {
                throw new DOMException("", "TypeMismatchError");
            }
            return found;
        }
        if (!create) {
            throw new DOMException("", "NotFoundError");
        }
        const child = made();
        this.children.set(name, child);
        return child;
    }

    async getDirectoryHandle(name, options) {
        return this.child(name, "directory", options?.create, () => new MemoryDirectory());
    }

    async getFileHandle(name, options) {
        return this.child(name, "file", options?.create, () => new MemoryFile());
    }

    async removeEntry(name) {
        const found = this.children.get(name);
        if (!found) {
            throw new DOMException("", "NotFoundError");
        }
        if (found.kind === "directory" && found.children.size > 0) {
            throw new DOMException("", "InvalidModificationError");
        }
        this.children.delete(name);
    }

    async *entries() {
        yield* [...this.children.entries()];
    }
}

/** @type {FileSystemDirectoryHandle | MemoryDirectory} */
let root;

// Open sync access handles by path, and the descriptors open on each.
/** @type {Map<string, {access: FileSystemSyncAccessHandle, open: number, used: number}>} */
const handles = new Map();
let tick = 0;

// Descriptors from 4 up; 3 is the preopened root.
/** @type {Map<number, object>} */
const descriptors = new Map([[PREOPEN, { kind: "directory", parts: [] }]]);
let nextDescriptor = PREOPEN + 1;

function keyOf(parts) {
    return parts.join("/");
}

function view() {
    return new DataView(memory.buffer);
}

function text(pointer, length) {
    return new TextDecoder().decode(new Uint8Array(memory.buffer, pointer, length).slice());
}

function resolved(base, path) {
    const parts = [...base];
    for (const part of path.split("/")) {
        if (part === "" || part === ".") {
            continue;
        }
        if (part === "..") {
            parts.pop();
        } else {
            parts.push(part);
        }
    }
    return parts;
}

function directoryDescriptor(fd) {
    const found = descriptors.get(fd);
    if (!found) {
        throw new Failure(EBADF);
    }
    if (found.kind !== "directory") {
        throw new Failure(ENOTDIR);
    }
    return found;
}

function pathAt(fd, pointer, length) {
    return resolved(directoryDescriptor(fd).parts, text(pointer, length));
}

async function directory(parts, create = false) {
    let at = root;
    for (const part of parts) {
        at = await at.getDirectoryHandle(part, { create });
    }
    return at;
}

// What is at a path: a directory or a file, or null when nothing is.
async function entry(parts) {
    if (parts.length === 0) {
        return root;
    }
    let parent;
    try {
        parent = await directory(parts.slice(0, -1));
    } catch (error) {
        if (errnoOf(error) === ENOENT) {
            return null;
        }
        throw error;
    }
    const name = parts[parts.length - 1];
    try {
        return await parent.getFileHandle(name);
    } catch (error) {
        if (error?.name === "NotFoundError") {
            return null;
        }
        if (error?.name !== "TypeMismatchError") {
            throw error;
        }
    }
    return await parent.getDirectoryHandle(name);
}

function closeIdle() {
    const idle = [...handles.entries()].filter(([, held]) => held.open === 0);
    if (idle.length <= IDLE_HANDLES) {
        return;
    }
    idle.sort((left, right) => left[1].used - right[1].used);
    for (const [key, held] of idle.slice(0, idle.length - IDLE_HANDLES)) {
        held.access.close();
        handles.delete(key);
    }
}

async function access(parts, file) {
    const key = keyOf(parts);
    let held = handles.get(key);
    if (!held) {
        held = { access: await file.createSyncAccessHandle(), open: 0, used: 0 };
        handles.set(key, held);
    }
    held.used = ++tick;
    return held;
}

// A path is about to stop naming what it names: its handle, and those of
// everything below it, are closed first, since a file with an open handle can
// be neither removed nor moved.
function release(parts) {
    const key = keyOf(parts);
    for (const [held, value] of [...handles.entries()]) {
        if (held === key || held.startsWith(key + "/")) {
            if (value.open > 0) {
                throw new Failure(EBUSY);
            }
            value.access.close();
            handles.delete(held);
        }
    }
}

function fileDescriptor(fd) {
    const found = descriptors.get(fd);
    if (!found) {
        throw new Failure(EBADF);
    }
    if (found.kind !== "file") {
        throw new Failure(EISDIR);
    }
    found.held.used = ++tick;
    return found;
}

function* iovecs(pointer, count) {
    const data = view();
    for (let index = 0; index < count; index++) {
        const at = pointer + index * 8;
        yield [data.getUint32(at, true), data.getUint32(at + 4, true)];
    }
}

function readAt(file, pointer, count, offset) {
    let total = 0;
    for (const [buffer, length] of iovecs(pointer, count)) {
        const scratch = new Uint8Array(length);
        const read = file.held.access.read(scratch, { at: offset + total });
        new Uint8Array(memory.buffer, buffer, read).set(scratch.subarray(0, read));
        total += read;
        if (read < length) {
            break;
        }
    }
    return total;
}

function writeAt(file, pointer, count, offset) {
    let total = 0;
    for (const [buffer, length] of iovecs(pointer, count)) {
        const scratch = new Uint8Array(memory.buffer, buffer, length).slice();
        total += file.held.access.write(scratch, { at: offset + total });
    }
    return total;
}

function writeFilestat(pointer, filetype, size) {
    const data = view();
    for (let at = 0; at < 64; at += 8) {
        data.setBigUint64(pointer + at, 0n, true);
    }
    data.setUint8(pointer + 16, filetype);
    data.setBigUint64(pointer + 24, 1n, true);
    data.setBigUint64(pointer + 32, BigInt(size), true);
}

const calls = {
    fd_prestat_get(fd, pointer) {
        if (fd !== PREOPEN) {
            return EBADF;
        }
        const data = view();
        data.setUint8(pointer, 0);
        data.setUint32(pointer + 4, 1, true);
        return SUCCESS;
    },

    fd_prestat_dir_name(fd, pointer, length) {
        if (fd !== PREOPEN) {
            return EBADF;
        }
        if (length < 1) {
            return EINVAL;
        }
        new Uint8Array(memory.buffer, pointer, 1)[0] = "/".charCodeAt(0);
        return SUCCESS;
    },

    async path_open(fd, dirflags, path, pathLength, oflags, base, inheriting, fdflags, opened) {
        const parts = pathAt(fd, path, pathLength);
        let found = await entry(parts);
        if (found && oflags & OFLAGS_CREAT && oflags & OFLAGS_EXCL) {
            return EEXIST;
        }
        if (!found) {
            if (!(oflags & OFLAGS_CREAT) || oflags & OFLAGS_DIRECTORY || parts.length === 0) {
                return ENOENT;
            }
            const parent = await directory(parts.slice(0, -1));
            found = await parent.getFileHandle(parts[parts.length - 1], { create: true });
        }
        let opening;
        if (found.kind === "directory") {
            if (oflags & OFLAGS_TRUNC) {
                return EISDIR;
            }
            opening = { kind: "directory", parts };
        } else {
            if (oflags & OFLAGS_DIRECTORY) {
                return ENOTDIR;
            }
            const held = await access(parts, found);
            if (oflags & OFLAGS_TRUNC) {
                held.access.truncate(0);
            }
            held.open += 1;
            opening = { kind: "file", parts, held, position: 0, append: (fdflags & FDFLAGS_APPEND) !== 0 };
        }
        const descriptor = nextDescriptor++;
        descriptors.set(descriptor, opening);
        view().setUint32(opened, descriptor, true);
        return SUCCESS;
    },

    fd_close(fd) {
        const found = descriptors.get(fd);
        if (!found || fd === PREOPEN) {
            return EBADF;
        }
        descriptors.delete(fd);
        if (found.kind === "file") {
            found.held.open -= 1;
            closeIdle();
        }
        return SUCCESS;
    },

    fd_read(fd, iovs, count, read) {
        const file = fileDescriptor(fd);
        const total = readAt(file, iovs, count, file.position);
        file.position += total;
        view().setUint32(read, total, true);
        return SUCCESS;
    },

    fd_pread(fd, iovs, count, offset, read) {
        const file = fileDescriptor(fd);
        view().setUint32(read, readAt(file, iovs, count, offset), true);
        return SUCCESS;
    },

    fd_write(fd, iovs, count, written) {
        const file = fileDescriptor(fd);
        if (file.append) {
            file.position = file.held.access.getSize();
        }
        const total = writeAt(file, iovs, count, file.position);
        file.position += total;
        view().setUint32(written, total, true);
        return SUCCESS;
    },

    fd_pwrite(fd, iovs, count, offset, written) {
        const file = fileDescriptor(fd);
        view().setUint32(written, writeAt(file, iovs, count, offset), true);
        return SUCCESS;
    },

    fd_seek(fd, offset, whence, position) {
        const file = fileDescriptor(fd);
        const from = [0, file.position, file.held.access.getSize()][whence];
        if (from === undefined || from + offset < 0) {
            return EINVAL;
        }
        file.position = from + offset;
        view().setBigUint64(position, BigInt(file.position), true);
        return SUCCESS;
    },

    fd_tell(fd, position) {
        view().setBigUint64(position, BigInt(fileDescriptor(fd).position), true);
        return SUCCESS;
    },

    fd_sync(fd) {
        const found = descriptors.get(fd);
        if (!found) {
            return EBADF;
        }
        if (found.kind === "file") {
            found.held.access.flush();
        }
        return SUCCESS;
    },

    fd_datasync(fd) {
        return calls.fd_sync(fd);
    },

    fd_fdstat_get(fd, pointer) {
        const found = descriptors.get(fd);
        if (!found) {
            return EBADF;
        }
        const data = view();
        data.setUint8(pointer, found.kind === "file" ? FILETYPE_REGULAR_FILE : FILETYPE_DIRECTORY);
        data.setUint16(pointer + 2, found.append ? FDFLAGS_APPEND : 0, true);
        // Every right, so that wasi-libc hands a descriptor opened under the
        // root whatever rights it asks for.
        data.setBigUint64(pointer + 8, 0xffffffffffffffffn, true);
        data.setBigUint64(pointer + 16, 0xffffffffffffffffn, true);
        return SUCCESS;
    },

    fd_fdstat_set_flags(fd, flags) {
        const found = descriptors.get(fd);
        if (!found) {
            return EBADF;
        }
        found.append = (flags & FDFLAGS_APPEND) !== 0;
        return SUCCESS;
    },

    fd_filestat_get(fd, pointer) {
        const found = descriptors.get(fd);
        if (!found) {
            return EBADF;
        }
        if (found.kind === "file") {
            writeFilestat(pointer, FILETYPE_REGULAR_FILE, found.held.access.getSize());
        } else {
            writeFilestat(pointer, FILETYPE_DIRECTORY, 0);
        }
        return SUCCESS;
    },

    fd_filestat_set_size(fd, size) {
        fileDescriptor(fd).held.access.truncate(size);
        return SUCCESS;
    },

    fd_filestat_set_times() {
        return SUCCESS;
    },

    fd_advise() {
        return SUCCESS;
    },

    fd_allocate(fd, offset, length) {
        const file = fileDescriptor(fd);
        if (file.held.access.getSize() < offset + length) {
            file.held.access.truncate(offset + length);
        }
        return SUCCESS;
    },

    async fd_readdir(fd, pointer, length, cookie, used) {
        const found = directoryDescriptor(fd);
        if (cookie === 0 || !found.listed) {
            found.listed = [];
            for await (const [name, handle] of (await directory(found.parts)).entries()) {
                found.listed.push([name, handle.kind === "directory" ? FILETYPE_DIRECTORY : FILETYPE_REGULAR_FILE]);
            }
        }
        const data = view();
        const encoder = new TextEncoder();
        let at = 0;
        for (let index = cookie; index < found.listed.length && at < length; index++) {
            const [name, filetype] = found.listed[index];
            const encoded = encoder.encode(name);
            const header = new DataView(new ArrayBuffer(24));
            header.setBigUint64(0, BigInt(index + 1), true);
            header.setBigUint64(8, BigInt(index + 1), true);
            header.setUint32(16, encoded.length, true);
            header.setUint8(20, filetype);
            const record = new Uint8Array(24 + encoded.length);
            record.set(new Uint8Array(header.buffer), 0);
            record.set(encoded, 24);
            // A record that does not fit is cut short, which tells wasi-libc to
            // ask again with a larger buffer.
            const fits = Math.min(record.length, length - at);
            new Uint8Array(memory.buffer, pointer + at, fits).set(record.subarray(0, fits));
            at += fits;
        }
        data.setUint32(used, at, true);
        return SUCCESS;
    },

    async path_filestat_get(fd, flags, path, pathLength, pointer) {
        const parts = pathAt(fd, path, pathLength);
        const found = await entry(parts);
        if (!found) {
            return ENOENT;
        }
        if (found.kind === "directory") {
            writeFilestat(pointer, FILETYPE_DIRECTORY, 0);
            return SUCCESS;
        }
        const held = handles.get(keyOf(parts));
        const size = held ? held.access.getSize() : (await found.getFile()).size;
        writeFilestat(pointer, FILETYPE_REGULAR_FILE, size);
        return SUCCESS;
    },

    path_filestat_set_times() {
        return SUCCESS;
    },

    async path_create_directory(fd, path, pathLength) {
        const parts = pathAt(fd, path, pathLength);
        if (parts.length === 0 || (await entry(parts))) {
            return EEXIST;
        }
        await (await directory(parts.slice(0, -1))).getDirectoryHandle(parts[parts.length - 1], {
            create: true,
        });
        return SUCCESS;
    },

    async path_remove_directory(fd, path, pathLength) {
        const parts = pathAt(fd, path, pathLength);
        const found = await entry(parts);
        if (!found) {
            return ENOENT;
        }
        if (found.kind !== "directory" || parts.length === 0) {
            return ENOTDIR;
        }
        await (await directory(parts.slice(0, -1))).removeEntry(parts[parts.length - 1]);
        return SUCCESS;
    },

    async path_unlink_file(fd, path, pathLength) {
        const parts = pathAt(fd, path, pathLength);
        const found = await entry(parts);
        if (!found) {
            return ENOENT;
        }
        if (found.kind !== "file") {
            return EISDIR;
        }
        release(parts);
        await (await directory(parts.slice(0, -1))).removeEntry(parts[parts.length - 1]);
        return SUCCESS;
    },

    async path_rename(fd, path, pathLength, toFd, toPath, toPathLength) {
        const from = pathAt(fd, path, pathLength);
        const to = pathAt(toFd, toPath, toPathLength);
        if (from.length === 0 || to.length === 0) {
            return EBUSY;
        }
        const found = await entry(from);
        if (!found) {
            return ENOENT;
        }
        if (keyOf(from) === keyOf(to)) {
            return SUCCESS;
        }
        if (found.kind !== "file") {
            // Nothing the app does renames a directory.
            return ENOTSUP;
        }
        const replaced = await entry(to);
        if (replaced?.kind === "directory") {
            return EISDIR;
        }
        release(from);
        const target = await directory(to.slice(0, -1));
        const name = to[to.length - 1];
        if (replaced) {
            release(to);
            await target.removeEntry(name);
        }
        if (typeof found.move === "function") {
            await found.move(target, name);
            return SUCCESS;
        }
        const source = await found.createSyncAccessHandle();
        const copy = await (await target.getFileHandle(name, { create: true })).createSyncAccessHandle();
        try {
            const bytes = new Uint8Array(source.getSize());
            source.read(bytes, { at: 0 });
            copy.truncate(0);
            copy.write(bytes, { at: 0 });
            copy.flush();
        } finally {
            source.close();
            copy.close();
        }
        await (await directory(from.slice(0, -1))).removeEntry(from[from.length - 1]);
        return SUCCESS;
    },

    path_readlink() {
        return EINVAL;
    },

    path_symlink() {
        return ENOTSUP;
    },

    path_link() {
        return ENOTSUP;
    },
};

for (const name of FILE_CALLS) {
    if (!(name in calls)) {
        throw new Error(`files.js does not answer ${name}, which wasi.js forwards`);
    }
}


async function answer(control, args) {
    await memoryReady;
    const name = FILE_CALLS[control[CONTROL.CALL]];
    const count = calls[name].length;
    const values = [];
    for (let index = 0; index < count; index++) {
        values.push(Number(args[index]));
    }
    try {
        return await calls[name](...values);
    } catch (error) {
        return errnoOf(error);
    }
}

async function serve(control, args) {
    for (;;) {
        const state = Atomics.load(control, CONTROL.STATE);
        if (state !== CONTROL.ASKED) {
            if (typeof Atomics.waitAsync === "function") {
                const waiting = Atomics.waitAsync(control, CONTROL.STATE, state);
                if (waiting.async) {
                    await waiting.value;
                }
            } else {
                await new Promise((resolve) => setTimeout(resolve, 1));
            }
            continue;
        }
        control[CONTROL.RESULT] = await answer(control, args);
        Atomics.store(control, CONTROL.STATE, CONTROL.ANSWERED);
        Atomics.notify(control, CONTROL.STATE);
    }
}

async function openRoot() {
    try {
        const opfs = await navigator.storage.getDirectory();
        // The probe a private window fails: it can list the directory but
        // cannot open a file in it for synchronous access.
        const probe = await opfs.getFileHandle(".probe", { create: true });
        (await probe.createSyncAccessHandle()).close();
        await opfs.removeEntry(".probe");
        return { root: opfs, persistent: true };
    } catch (error) {
        console.warn("the origin private file system is unavailable, so nothing is saved", error);
        return { root: new MemoryDirectory(), persistent: false };
    }
}

self.onmessage = async (event) => {
    if (event.data.memory) {
        memory = event.data.memory;
        memoryBound();
        return;
    }
    const { control, args } = event.data;
    const opened = await openRoot();
    root = opened.root;
    self.postMessage({ ready: true, persistent: opened.persistent });
    serve(new Int32Array(control), new BigInt64Array(args));
};
