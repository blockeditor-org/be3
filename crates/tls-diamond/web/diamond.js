// The thread-local diamond (crates/tls-diamond) in a browser: the plugin and
// the libraries it needs, base, left and right, booted by block-app's
// plugin.js in this worker as a plugin's worker boots one. The plugin checks
// its thread-locals on the main thread and on a thread it spawns, which wakes
// it once it is done; the next step sends [1] if all of them were right.

import { boot } from "./bundle/plugin.js";

async function run() {
    let woken;
    const wake = new Promise((resolve) => {
        woken = resolve;
    });
    const plugin = await boot(
        new URL("./bundle/block_gpu_shim.js", import.meta.url).href,
        new URL("./bundle/plugin.wasm", import.meta.url).href,
        () => woken(),
        [],
    );
    await wake;
    plugin.step();
    const frames = plugin.collect().map((frame) => Array.from(frame));
    if (JSON.stringify(frames) !== "[[1]]") {
        return { error: `the plugin sent ${JSON.stringify(frames)}, not [[1]]` };
    }
    return { passed: true, message: "the thread-local diamond ran in Chromium" };
}

self.onmessage = async () => {
    try {
        self.postMessage(await run());
    } catch (error) {
        self.postMessage({ error: String(error?.stack ?? error) });
    }
};
