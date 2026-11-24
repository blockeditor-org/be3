// Draws a browser painting in Chromium through Playwright: main.rs writes the painting to stdin as
// JSON (beui-paint-snapshot's BrowserSnapshot, with beui's fonts added), and this writes the PNG to
// stdout. Playwright is the node_modules beside it, and the browser BE3_CHROME (main.rs sets both).

import { createRequire } from "node:module";

const { chromium } = createRequire(import.meta.url)("playwright");

const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const snapshot = JSON.parse(Buffer.concat(chunks).toString("utf8"));

const origin = "https://snapshot.invalid/";
const withoutFragment = (url) => {
    const parsed = new URL(url, origin);
    parsed.hash = "";
    return parsed.href;
};
const files = new Map(
    Object.entries(snapshot.files).map(([url, file]) => [
        withoutFragment(url),
        {
            contentType: file.content_type,
            body: file.text != null ? Buffer.from(file.text.join(""), "utf8") : Buffer.from(file.bytes),
        },
    ]),
);
files.set(origin, { contentType: "text/html; charset=utf-8", body: Buffer.from(snapshot.document.join(""), "utf8") });
const missing = new Set();
const errors = [];

const browser = await chromium.launch({ executablePath: process.env.BE3_CHROME });
let image;
let broken;
try {
    const context = await browser.newContext({
        viewport: { width: snapshot.size[0], height: snapshot.size[1] },
        deviceScaleFactor: 1,
        locale: "en-US",
        timezoneId: "UTC",
        colorScheme: "light",
        reducedMotion: "reduce",
        serviceWorkers: "block",
        offline: true,
    });
    await context.route("**/*", (route) => {
        const url = withoutFragment(route.request().url());
        const resource = files.get(url);
        if (resource === undefined) {
            missing.add(url);
            return route.abort();
        }
        return route.fulfill({ ...resource, headers: { "access-control-allow-origin": "*" } });
    });
    const page = await context.newPage();
    page.on("pageerror", (error) => errors.push(String(error)));
    await page.goto(origin, { waitUntil: "load", timeout: 30_000 });
    await page.evaluate(() => document.fonts.ready);
    broken = await page.evaluate(() =>
        Promise.all(
            Array.from(document.images, async (image) => {
                try {
                    await image.decode();
                    return null;
                } catch {
                    return image.currentSrc || image.src;
                }
            }),
        ),
    );
    image = await page.screenshot({ animations: "disabled", caret: "hide", timeout: 30_000 });
} finally {
    await browser.close();
}

const fail = (message) => {
    process.stderr.write(`${message}\n`);
    process.exit(1);
};
if (missing.size) fail(`Snapshot is missing embedded resources:\n${[...missing].sort().join("\n")}`);
if (broken.some(Boolean)) fail(`Snapshot images failed to decode:\n${broken.filter(Boolean).join("\n")}`);
if (errors.length) fail(`Snapshot scripts failed:\n${errors.join("\n")}`);
process.stdout.write(image);
