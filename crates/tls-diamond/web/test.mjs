// Runs the thread-local diamond (diamond.js) in Chromium through Playwright:
//
//   node test.mjs DIR
//
// DIR holds index.html, diamond.js, block-app's dylink.js and wasi.js, and
// base.wasm, left.wasm, right.wasm and plugin.wasm. The browser is BE3_CHROME
// when set, else Playwright's Chromium, as for buck/app/web-test.mjs.

import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const TIMEOUT = 60_000;
const directory = resolve(process.argv[2] ?? ".");

function playwright() {
    const require = createRequire(import.meta.url);
    const places = [undefined];
    try {
        places.push(execFileSync("npm", ["root", "-g"], { encoding: "utf8" }).trim());
    } catch {}
    for (const place of places) {
        try {
            return require(place ? join(place, "playwright") : "playwright");
        } catch {}
    }
    console.error("Playwright is missing: install it with `npm install -g playwright`.");
    process.exit(1);
}

const server = createServer(async (request, response) => {
    const path = normalize(decodeURIComponent(new URL(request.url, "http://localhost").pathname));
    const file = join(directory, path === "/" ? "index.html" : path);
    try {
        const body = await readFile(file);
        response.writeHead(200, {
            "Content-Type": TYPES[extname(file)] ?? "application/octet-stream",
            "Cross-Origin-Opener-Policy": "same-origin",
            "Cross-Origin-Embedder-Policy": "require-corp",
        });
        response.end(body);
    } catch {
        response.writeHead(404).end();
    }
});
await new Promise((ready) => server.listen(0, "127.0.0.1", ready));

// Chromium's singleton socket lives in TMPDIR, and buck's is too long a path
// for a Unix socket.
const temporary = await mkdtemp("/tmp/be3-tls-diamond-");
const { chromium } = playwright();
const browser = await chromium.launch({
    executablePath: process.env.BE3_CHROME || chromium.executablePath(),
    args: ["--no-sandbox"],
    env: { ...process.env, TMPDIR: temporary },
});
let failed = true;
try {
    const page = await browser.newPage();
    page.on("console", (message) => console.log(`[${message.type()}] ${message.text()}`));
    page.on("pageerror", (error) => console.log(`[pageerror] ${error}`));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const outcome = await page.evaluate(
        (timeout) =>
            Promise.race([
                globalThis.diamond,
                new Promise((resolve) => setTimeout(() => resolve({ error: "timed out" }), timeout)),
            ]),
        TIMEOUT,
    );
    if (outcome.result === 1) {
        console.log("the thread-local diamond ran in Chromium");
        failed = false;
    } else {
        console.error(`the thread-local diamond failed: ${outcome.error ?? JSON.stringify(outcome)}`);
    }
} finally {
    await browser.close();
    server.close();
    await rm(temporary, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
