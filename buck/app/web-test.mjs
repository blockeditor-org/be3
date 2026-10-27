// Runs drive scripts against a web bundle on beui's HTML core, in Chromium
// through Playwright: web_drive_test in drive.bzl, and guides/testing_a_gui.md.
//
//   node web-test.mjs --bundle=DIR [--checks] SCRIPT.drive... [--nocapture] [NAME]
//
// It serves DIR, and for each script opens DIR's index.html with
// ?fixture=<the script's file name> and runs the script's commands, one a line
// as `drive -` reads them. The page answers the reading commands through its
// automate(words) (beui-core-html's automate: tree, ids, find, wait, gone,
// settle, pause, clock); click, type and key are real Playwright input, at the
// place `find` answers, so the browser's own focus, key and text paths run.
// --checks also opens ?checks, once in each colour scheme, and fails on any
// line of the page's checks() that is not "ok". NAME runs only the scripts and checks whose names contain
// it. The browser is BE3_CHROME when set, else Playwright's Chromium; with
// neither, it fails rather than skipping.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { basename, extname, join, normalize, resolve } from "node:path";

import { chord, commands, leadingOptions } from "./drive-words.mjs";

const SIZE = { width: 1100, height: 720 };
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

let bundle;
let checks = false;
let filter = "";
const scripts = [];
for (const argument of process.argv.slice(2)) {
    if (argument.startsWith("--bundle=")) {
        bundle = resolve(argument.slice("--bundle=".length));
    } else if (argument === "--checks") {
        checks = true;
    } else if (argument === "--nocapture") {
        continue;
    } else if (argument.endsWith(".drive")) {
        scripts.push(argument);
    } else {
        filter = argument;
    }
}
if (!bundle) {
    console.error("usage: web-test.mjs --bundle=DIR [--checks] SCRIPT.drive... [NAME]");
    process.exit(1);
}

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
    console.error(
        "Playwright is missing: install it with `npm install -g playwright`. The browser tests need it and fail without it.",
    );
    process.exit(1);
}

const { chromium } = playwright();
const executablePath = process.env.BE3_CHROME || chromium.executablePath();
if (!executablePath || !existsSync(executablePath)) {
    console.error(
        `No browser: ${executablePath || "Playwright names no Chromium"} is not there. Set BE3_CHROME to a Chrome or Chromium, or run \`npx playwright install chromium\`.`,
    );
    process.exit(1);
}

const server = createServer(async (request, response) => {
    const path = normalize(decodeURIComponent(new URL(request.url, "http://localhost").pathname));
    const file = join(bundle, path === "/" ? "index.html" : path);
    if (!file.startsWith(bundle)) {
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
const origin = `http://127.0.0.1:${server.address().port}/`;

// Chromium's singleton socket lives in TMPDIR, and buck's is too long a path
// for a Unix socket.
const temporary = await mkdtemp("/tmp/be3-web-test-");
const browser = await chromium.launch({
    executablePath,
    args: ["--no-sandbox"],
    env: { ...process.env, TMPDIR: temporary },
});

async function open(query, colorScheme = "light") {
    const context = await browser.newContext({ viewport: SIZE, colorScheme });
    const page = await context.newPage();
    const log = [];
    const errors = [];
    page.on("console", (message) => log.push(`[${message.type()}] ${message.text()}`));
    page.on("pageerror", (error) => errors.push(String(error?.stack ?? error)));
    await page.goto(origin + query);
    await page.waitForFunction(() => globalThis.beuiStarted !== undefined);
    const started = await page.evaluate(async () => {
        try {
            await globalThis.beuiStarted;
            return null;
        } catch (error) {
            return String(error?.stack ?? error);
        }
    });
    if (started) errors.push(`the page did not start: ${started}`);
    return { context, page, log, errors };
}

async function automate(page, words) {
    return page.evaluate(async (words) => {
        try {
            const tests = await globalThis.beuiStarted;
            return { text: await tests.automate(words) };
        } catch (error) {
            return { error: String(error?.message ?? error) };
        }
    }, words);
}

class Failure extends Error {}

async function answer(page, words) {
    const { text, error } = await automate(page, words);
    if (error !== undefined) throw new Failure(error);
    return text;
}

async function center(page, target, timeout) {
    await answer(page, [`--timeout=${timeout}`, "wait", target.startsWith("#") ? target.split("+")[0] : target]);
    const reply = await answer(page, ["find", target]);
    const found = reply.match(/at (-?[\d.]+),(-?[\d.]+) size ([\d.]+)x([\d.]+)/);
    if (!found) throw new Failure(`${target} has no place on screen: ${reply.trim()}`);
    const ratio = await page.evaluate(() => devicePixelRatio);
    const [x, y, width, height] = found.slice(1).map((value) => Number(value) / ratio);
    return { x: x + width / 2, y: y + height / 2 };
}

async function step(page, words) {
    const { timeout, options, rest } = leadingOptions(words);
    const [command, ...args] = rest;
    const settle = () => answer(page, [`--timeout=${timeout}`, "settle"]);
    switch (command) {
        case "click": {
            const buttons = { right: ["right", 1], middle: ["middle", 1], double: ["left", 2] };
            const [button, clickCount] = buttons[args[1]] ?? ["left", 1];
            if (args[1] && !buttons[args[1]]) throw new Failure(`click takes right, middle or double, not ${args[1]}`);
            const { x, y } = await center(page, args[0], timeout);
            await page.mouse.move(x, y);
            await settle();
            await page.mouse.click(x, y, { button, clickCount });
            return settle();
        }
        case "move": {
            const { x, y } = await center(page, args[0], timeout);
            await page.mouse.move(x, y);
            return settle();
        }
        case "drag": {
            const from = await center(page, args[0], timeout);
            const to = await center(page, args[1], timeout);
            const steps = Number(args[2] ?? 2);
            await page.mouse.move(from.x, from.y);
            await page.mouse.down();
            for (let at = 1; at <= steps; at += 1) {
                const part = at / steps;
                await page.mouse.move(from.x + (to.x - from.x) * part, from.y + (to.y - from.y) * part);
                await settle();
            }
            await page.mouse.up();
            return settle();
        }
        case "type":
            await page.keyboard.type(args.join(" "));
            return settle();
        case "key":
            for (const pressed of args) await page.keyboard.press(chord(pressed));
            return settle();
        default:
            return answer(page, [...options, `--timeout=${timeout}`, command, ...args]);
    }
}

async function close(opened) {
    await opened.context.close();
}

async function report(opened, failure) {
    console.log(`FAILED: ${failure}`);
    for (const error of opened.errors) console.log(`page error: ${error}`);
    try {
        const tree = await automate(opened.page, ["tree"]);
        if (tree.text) console.log(`the tree:\n${tree.text}`);
    } catch {}
    if (opened.log.length) console.log(`the page's console:\n${opened.log.join("\n")}`);
}

async function runScript(script) {
    const name = basename(script, ".drive");
    console.log(`\n${name} (${script})`);
    const opened = await open(`?fixture=${encodeURIComponent(name)}`);
    try {
        if (opened.errors.length) throw new Failure(opened.errors[0]);
        for (const { line, words } of commands(await readFile(script, "utf8"))) {
            console.log(`> ${line}`);
            const output = await step(opened.page, words);
            if (output) process.stdout.write(output.endsWith("\n") ? output : `${output}\n`);
            if (opened.errors.length) throw new Failure(`the page failed: ${opened.errors[0]}`);
        }
        console.log(`ok ${name}`);
        return true;
    } catch (error) {
        await report(opened, error instanceof Failure ? error.message : String(error?.stack ?? error));
        return false;
    } finally {
        await close(opened);
    }
}

async function runChecks(colorScheme) {
    console.log(`\nchecks (${colorScheme})`);
    const opened = await open("?checks", colorScheme);
    try {
        if (opened.errors.length) throw new Failure(opened.errors[0]);
        await answer(opened.page, ["settle"]);
        const lines = await opened.page.evaluate(async () => (await globalThis.beuiStarted).checks());
        const chosen = lines
            .trim()
            .split("\n")
            .filter((line) => line.includes(filter));
        process.stdout.write(chosen.map((line) => `${line}\n`).join(""));
        if (opened.errors.length) throw new Failure(`the page failed: ${opened.errors[0]}`);
        if (chosen.some((line) => !line.startsWith("ok "))) {
            if (opened.log.length) console.log(`the page's console:\n${opened.log.join("\n")}`);
            return false;
        }
        return true;
    } catch (error) {
        await report(opened, error instanceof Failure ? error.message : String(error?.stack ?? error));
        return false;
    } finally {
        await close(opened);
    }
}

let passed = true;
let ran = 0;
for (const script of scripts) {
    if (!basename(script, ".drive").includes(filter)) continue;
    ran += 1;
    passed = (await runScript(script)) && passed;
}
if (checks) {
    for (const colorScheme of ["light", "dark"]) {
        ran += 1;
        passed = (await runChecks(colorScheme)) && passed;
    }
}
await browser.close();
server.close();
await rm(temporary, { recursive: true, force: true });
if (ran === 0) {
    console.log(`no script or check has a name containing ${filter}`);
}
process.exit(passed ? 0 : 1);
