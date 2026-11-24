// What the browser drivers here share (page-test.mjs, web-test.mjs and web-host-test.mjs): Chromium
// through Playwright, and a static server for a directory.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { extname, join, normalize } from "node:path";

const TYPES = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".mjs": "text/javascript",
    ".json": "application/json",
    ".wasm": "application/wasm",
    ".ttf": "font/ttf",
    ".woff2": "font/woff2",
    ".css": "text/css",
};

// Playwright's `chromium`: from NODE_PATH, which names buck/tools:playwright-modules on a worker,
// else from npm's global modules.
function playwright() {
    const require = createRequire(import.meta.url);
    const places = [undefined];
    try {
        places.push(execFileSync("npm", ["root", "-g"], { encoding: "utf8" }).trim());
    } catch {}
    for (const place of places) {
        try {
            return require(place ? join(place, "playwright") : "playwright").chromium;
        } catch {}
    }
    console.error(
        "Playwright is missing: NODE_PATH names it on a worker, or install it with `npm install -g playwright`. The browser tests fail without it.",
    );
    process.exit(1);
}

// Chromium, BE3_CHROME when set, else Playwright's own, with `args` added to running as root
// with no sandbox. Its singleton socket lives in TMPDIR, and buck's is too long a path for a Unix
// socket, so `temporary` is a short directory for it. It exits rather than skipping when there is
// no browser.
export async function launch({ args = [], temporary }) {
    const chromium = playwright();
    const executablePath = process.env.BE3_CHROME || chromium.executablePath();
    if (!executablePath || !existsSync(executablePath)) {
        console.error(
            `No browser: ${executablePath || "Playwright names no Chromium"} is not there. Set BE3_CHROME to a Chrome or Chromium, or run \`npx playwright install chromium\`.`,
        );
        process.exit(1);
    }
    return await chromium.launch({
        executablePath,
        args: ["--no-sandbox", "--disable-dev-shm-usage", ...args],
        env: { ...process.env, TMPDIR: temporary },
    });
}

// Serves `directory` on a free port of 127.0.0.1, `/` as its index.html, cross-origin isolated
// (the bundles' threads need SharedArrayBuffer) and never cached. It answers { origin, close },
// the origin without its trailing slash.
export async function serve(directory) {
    const server = createServer(async (request, response) => {
        const path = normalize(decodeURIComponent(new URL(request.url, "http://localhost").pathname));
        const file = join(directory, path === "/" ? "index.html" : path);
        if (!file.startsWith(directory)) {
            response.writeHead(403).end();
            return;
        }
        try {
            const body = await readFile(file);
            response.writeHead(200, {
                "Content-Type": TYPES[extname(file)] ?? "application/octet-stream",
                "Cross-Origin-Opener-Policy": "same-origin",
                "Cross-Origin-Embedder-Policy": "require-corp",
                "Cache-Control": "no-store",
            });
            response.end(body);
        } catch {
            response.writeHead(404).end();
        }
    });
    await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
    return { origin: `http://127.0.0.1:${server.address().port}`, close: () => server.close() };
}
