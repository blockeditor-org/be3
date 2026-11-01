// The thread-local diamond (crates/tls-diamond) in a browser: the plugin and
// the libraries it needs, base, left and right, run by block-guest-host's
// guest.js with block-app's plugin.js in this worker as a plugin's worker runs
// one. The plugin checks
// its thread-locals on the main thread and on a thread it spawns, which wakes
// it once it is done; the next step sends [1] if all of them were right. A
// second worker serves one of the libraries with a byte added, which plugin.js
// has to refuse, since it no longer hashes to its name.

import { runtime } from "./bundle/guest.js";

const START = {
    kind: "start",
    url: new URL("./bundle/plugin.wasm", import.meta.url).href,
    extension: new URL("./bundle/plugin.js", import.meta.url).href,
    imports: [],
};

async function diamond() {
    let woken;
    const wake = new Promise((resolve) => {
        woken = resolve;
    });
    const posted = [];
    const guest = runtime((message) => (message.kind === "wake" ? woken() : posted.push(message)));
    await guest.receive(START);
    const failed = () => posted.find(({ kind }) => kind === "error");
    if (failed()) {
        return { error: failed().message };
    }
    await wake;
    await guest.receive({ kind: "step", frames: [] });
    if (failed()) {
        return { error: failed().message };
    }
    const frames = posted
        .filter(({ kind }) => kind === "ready" || kind === "stepped")
        .flatMap((report) => report.frames)
        .map((frame) => Array.from(new Uint8Array(frame)));
    if (JSON.stringify(frames) !== "[[1]]") {
        return { error: `the plugin sent ${JSON.stringify(frames)}, not [[1]]` };
    }
    return { passed: true, message: "the thread-local diamond ran in Chromium" };
}

// An empty custom section after the library's own sections, which leaves it a
// valid module with another SHA-256.
async function tampered() {
    const fetched = self.fetch;
    self.fetch = async (url, ...rest) => {
        const response = await fetched(url, ...rest);
        if (!String(url).includes("/lib-")) {
            return response;
        }
        const bytes = new Uint8Array(await response.arrayBuffer());
        const changed = new Uint8Array(bytes.length + 3);
        changed.set(bytes);
        changed.set([0, 1, 0], bytes.length);
        return new Response(changed, { headers: { "content-type": "application/wasm" } });
    };
    const posted = [];
    await runtime((message) => posted.push(message)).receive(START);
    const failed = posted.find(({ kind }) => kind === "error");
    if (!failed) {
        return { error: "a library that is not what it is named was linked" };
    }
    return failed.message.includes("its SHA-256 is")
        ? { passed: true, message: "a library that is not what it is named was refused" }
        : { error: `the library was refused for another reason: ${failed.message}` };
}

self.onmessage = async ({ data }) => {
    try {
        self.postMessage(await (data.kind === "tampered" ? tampered() : diamond()));
    } catch (error) {
        self.postMessage({ error: String(error?.stack ?? error) });
    }
};
