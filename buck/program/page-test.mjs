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

import { mkdtemp, rm } from "node:fs/promises";
import { resolve } from "node:path";

import { launch, serve } from "./browser.mjs";

const TIMEOUT = 60_000;
const directory = resolve(process.argv[2] ?? ".");

const server = await serve(directory);
const temporary = await mkdtemp("/tmp/be3-page-test-");
const browser = await launch({ temporary });
let failed = true;
try {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.on("console", (message) => console.log(`[${message.type()}] ${message.text()}`));
    page.on("pageerror", (error) => console.log(`[pageerror] ${error}`));
    const { origin } = server;
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
