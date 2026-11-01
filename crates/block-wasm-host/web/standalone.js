// The standalone guest (src/tests/standalone.wat) in a browser: a plugin with
// a memory of its own that is not shared, no library, no WASI and no threads,
// run by block-guest-host's guest.js with block-app's plugin.js against
// block-gpu-shim in this worker, as a plugin's worker runs one. It paints its surface orange, which the shim
// exports as a bitmap that is read back here.

import { runtime } from "./bundle/guest.js";

const SURFACE = 7;

async function run() {
    const posted = [];
    const guest = runtime((message) => posted.push(message));
    await guest.receive({
        kind: "start",
        url: new URL("./standalone.wasm", import.meta.url).href,
        extension: new URL("./bundle/plugin.js", import.meta.url).href,
        imports: [],
    });
    await guest.receive({ kind: "export", surfaces: [SURFACE] });
    await guest.receive({ kind: "step", frames: [] });
    const failed = posted.find(({ kind }) => kind === "error");
    if (failed) {
        return { error: failed.message };
    }
    const stepped = posted.find(({ kind }) => kind === "stepped");
    const frames = posted.flatMap((report) => report.frames ?? []).map((frame) => Array.from(new Uint8Array(frame)));
    if (JSON.stringify(frames) !== "[[1]]") {
        return { error: `the plugin sent ${JSON.stringify(frames)}, not [[1]]` };
    }
    const picture = stepped.pictures.find(({ surface }) => surface === SURFACE);
    if (!picture) {
        return { error: "the plugin's surface was not exported once it presented" };
    }
    const canvas = new OffscreenCanvas(picture.bitmap.width, picture.bitmap.height);
    const context = canvas.getContext("2d");
    context.drawImage(picture.bitmap, 0, 0);
    const [red, green, blue, alpha] = context.getImageData(1, 1, 1, 1).data;
    const orange = red > 200 && green > 60 && green < 230 && blue < 60 && alpha > 200;
    if (!orange) {
        return { error: `the surface is ${[red, green, blue, alpha]}, not orange` };
    }
    return {
        passed: true,
        message: `a standalone plugin painted ${picture.bitmap.width} by ${picture.bitmap.height} in Chromium`,
    };
}

self.onmessage = async () => {
    try {
        self.postMessage(await run());
    } catch (error) {
        self.postMessage({ error: String(error?.stack ?? error) });
    }
};
