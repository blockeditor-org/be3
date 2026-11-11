// Runs a page that checks itself (page_test in drive.bzl) in Chromium through
// Playwright:
//
//   node page-test.mjs DIR
//
// DIR is served as it is, and its index.html sets globalThis.outcome to a
// promise of { passed: true, message } or { error }. The browser is BE3_CHROME
// when set, else Playwright's Chromium, as for buck/program/web-test.mjs.
// `await deliverPush(text)` delivers a push whose
// payload is `text` to its service worker, as a push service would (DevTools
// `ServiceWorker.deliverPushMessage`).

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
const temporary = await mkdtemp("/tmp/be3-page-test-");
const { chromium } = playwright();
const browser = await chromium.launch({
    executablePath: process.env.BE3_CHROME || chromium.executablePath(),
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
    env: { ...process.env, TMPDIR: temporary },
});
let failed = true;
try {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.on("console", (message) => console.log(`[${message.type()}] ${message.text()}`));
    page.on("pageerror", (error) => console.log(`[pageerror] ${error}`));
    const origin = `http://127.0.0.1:${server.address().port}`;
    const session = await context.newCDPSession(page);
    let registration;
    session.on("ServiceWorker.workerRegistrationUpdated", ({ registrations }) => {
        registration = registrations.at(-1)?.registrationId ?? registration;
    });
    await session.send("ServiceWorker.enable");
    await page.exposeFunction("deliverPush", async (data) => {
        if (!registration) {
            throw new Error("no service worker is registered");
        }
        await session.send("ServiceWorker.deliverPushMessage", { origin, registrationId: registration, data });
    });
    await page.goto(`${origin}/`);
    const outcome = await page.evaluate(
        (timeout) =>
            Promise.race([
                globalThis.outcome,
                new Promise((resolve) => setTimeout(() => resolve({ error: "timed out" }), timeout)),
            ]),
        TIMEOUT,
    );
    if (outcome?.passed === true) {
        console.log(outcome.message ?? "the page passed");
        failed = false;
    } else {
        console.error(`the page failed: ${outcome?.error ?? JSON.stringify(outcome)}`);
    }
} finally {
    await browser.close();
    server.close();
    await rm(temporary, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
