// Runs drive scripts against a web bundle on beui's HTML core, in Chromium
// through Playwright: web_drive_test in drive.bzl, and guides/testing_a_gui.md.
//
//   node web-test.mjs --bundle=DIR [--checks] SCRIPT.drive... [--nocapture] [NAME]
//
// It serves DIR, and for each script opens DIR's index.html with
// ?fixture=<the script's file name>, waits for it to settle and runs the script's commands, one a line
// as `drive -` reads them. The page answers the reading commands through its
// automate(words) (beui-core-html's automate: tree, ids, find, reveal, shown, wait, gone,
// settle, pause, clock); click, move, down, up, drag, wheel, type and key are real Playwright input, at the
// place `reveal` answers (the part of the target that shows, once a scroll that cuts it has
// revealed it; after a reveal every target of the command is found again, as native drive
// does), so the browser's own focus, key and text paths run.
// `selection none` and `selection some` (browser only) fail unless the page's
// text selection is empty, or holds some text.
// `role ROLE NAME [STATE...]` (browser only) waits until Playwright's getByRole finds exactly one
// visible element with that ARIA role and exact accessible name, in each STATE given (checked,
// disabled, expanded, modal, pressed, selected, each also as not-STATE, or level=N; modal reads
// Chrome's own accessibility tree, as the dialog's aria-modal reaches it).
// `roles ROLE COUNT` (browser only) waits until exactly COUNT visible elements have that ARIA
// role, whatever their names.
// After every command the document itself must not scroll or overflow the
// viewport (a beui app owns its viewport, and only its Scrolls scroll), and the
// browser must not draw its focus ring on what beui draws its own for.
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

import { Failure, chord, commands, leadingOptions, shownPlaces, wheelPixels } from "./drive-words.mjs";

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
    // A panic in wasm unwinds through a dozen frames of std before the code
    // that panicked, past the default limit of ten.
    await page.addInitScript(() => {
        Error.stackTraceLimit = 200;
    });
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
    else {
        // As a native program's launch does, start from a settled page: its
        // first frames may still be laying out what it built.
        const { error } = await automate(page, ["settle"]);
        if (error !== undefined) errors.push(`the page did not settle: ${error}`);
    }
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

async function answer(page, words) {
    const { text, error } = await automate(page, words);
    if (error !== undefined) throw new Failure(error);
    return text;
}

async function centers(page, targets, timeout) {
    const ratio = await page.evaluate(() => devicePixelRatio);
    const ask = async (command, target) => {
        await answer(page, [`--timeout=${timeout}`, "wait", target.startsWith("#") ? target.split("+")[0] : target]);
        return answer(page, [`--timeout=${timeout}`, command, target]);
    };
    const places = await shownPlaces(ask, targets, ratio);
    return places.map(({ x, y, width, height }) => ({ x: x + width / 2, y: y + height / 2 }));
}

async function center(page, target, timeout) {
    return (await centers(page, [target], timeout))[0];
}

const BUTTONS = { left: "left", right: "right", middle: "middle" };

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
            const [from, to] = await centers(page, args.slice(0, 2), timeout);
            let steps = 2;
            let button = "left";
            for (const extra of args.slice(2)) {
                if (BUTTONS[extra]) button = BUTTONS[extra];
                else if (/^\d+$/.test(extra)) steps = Number(extra);
                else throw new Failure(`drag takes a number of steps and a button, not ${extra}`);
            }
            await page.mouse.move(from.x, from.y);
            await page.mouse.down({ button });
            for (let at = 1; at <= steps; at += 1) {
                const part = at / steps;
                await page.mouse.move(from.x + (to.x - from.x) * part, from.y + (to.y - from.y) * part);
                await settle();
            }
            await page.mouse.up({ button });
            return settle();
        }
        case "down":
        case "up": {
            const named = args.filter((arg) => BUTTONS[arg]);
            const targets = args.filter((arg) => !BUTTONS[arg]);
            const button = BUTTONS[named[0]] ?? "left";
            if (targets[0] !== undefined) {
                const { x, y } = await center(page, targets[0], timeout);
                await page.mouse.move(x, y);
                await settle();
            } else if (command === "down") {
                throw new Failure("down needs a TARGET");
            }
            await (command === "down" ? page.mouse.down({ button }) : page.mouse.up({ button }));
            return settle();
        }
        case "wheel": {
            const { x, y } = await center(page, args[0], timeout);
            await page.mouse.move(x, y);
            await settle();
            await page.mouse.wheel(wheelPixels(args[2]), wheelPixels(args[1]));
            return settle();
        }
        case "resize": {
            const size = /^(\d+)x(\d+)$/.exec(args[0] ?? "");
            if (!size) throw new Failure(`resize takes WIDTHxHEIGHT, not ${args[0]}`);
            await page.setViewportSize({ width: Number(size[1]), height: Number(size[2]) });
            return settle();
        }
        case "type":
            await page.keyboard.type(args.join(" "));
            return settle();
        case "key":
            for (const pressed of args) await page.keyboard.press(chord(pressed));
            return settle();
        case "selection": {
            const wanted = { none: false, some: true }[args[0]];
            if (wanted === undefined) throw new Failure(`selection takes none or some, not ${args[0]}`);
            const selected = await page.evaluate(() => String(getSelection() ?? ""));
            if ((selected.trim() !== "") !== wanted) {
                throw new Failure(selected ? `the page selected ${JSON.stringify(selected)}` : "the page selected nothing");
            }
            return "";
        }
        case "role": {
            const [role, name, ...states] = args;
            if (!role || name === undefined) throw new Failure("role takes a ROLE, a NAME and any STATEs");
            const query = { name, exact: true };
            let modal;
            for (const state of states) {
                const level = state.match(/^level=(\d+)$/);
                const [key, value] = level ? ["level", Number(level[1])]
                    : state.startsWith("not-") ? [state.slice(4), false] : [state, true];
                if (!["checked", "disabled", "expanded", "modal", "pressed", "selected", "level"].includes(key)) {
                    throw new Failure(`role takes checked, disabled, expanded, modal, pressed, selected or level=N, not ${state}`);
                }
                if (key === "modal") modal = value;
                else query[key] = value;
            }
            const found = page.getByRole(role, query);
            try {
                await found.waitFor({ timeout: timeout * 1000 });
            } catch (error) {
                throw new Failure(`getByRole(${JSON.stringify(role)}, ${JSON.stringify(query)}): ${error.message.split("\n")[0]}`);
            }
            if (modal !== undefined && await axModal(page, found) !== modal) {
                throw new Failure(`${role} ${JSON.stringify(name)} is ${modal ? "not " : ""}modal in Chrome's accessibility tree`);
            }
            return "";
        }
        case "roles": {
            const [role, wanted] = args;
            const count = Number(wanted);
            if (!role || !Number.isInteger(count) || count < 0) throw new Failure("roles takes a ROLE and a COUNT");
            const found = page.getByRole(role);
            try {
                if (count > 0) await found.nth(count - 1).waitFor({ timeout: timeout * 1000 });
                await found.nth(count).waitFor({ state: "hidden", timeout: timeout * 1000 });
            } catch (error) {
                const shown = await found.count();
                throw new Failure(`getByRole(${JSON.stringify(role)}) found ${shown}, not ${count}: ${error.message.split("\n")[0]}`);
            }
            return "";
        }
        default:
            return answer(page, [...options, `--timeout=${timeout}`, command, ...args]);
    }
}

async function axModal(page, locator) {
    const mark = "data-web-test-modal";
    await locator.evaluate((element, mark) => element.setAttribute(mark, ""), mark);
    const session = await page.context().newCDPSession(page);
    try {
        const { root } = await session.send("DOM.getDocument", { depth: 0 });
        const { nodeId } = await session.send("DOM.querySelector", { nodeId: root.nodeId, selector: `[${mark}]` });
        const { nodes } = await session.send("Accessibility.getPartialAXTree", { nodeId, fetchRelatives: false });
        const modal = nodes[0]?.properties?.find(property => property.name === "modal");
        return modal?.value?.value === true;
    } finally {
        await session.detach();
        await locator.evaluate((element, mark) => element.removeAttribute(mark), mark);
    }
}

async function pageChecks(page) {
    const moved = await page.evaluate(() => {
        const root = document.scrollingElement;
        const extent = `${root.scrollWidth}x${root.scrollHeight} at ${root.scrollLeft},${root.scrollTop}`;
        const still = root.scrollTop === 0 && root.scrollLeft === 0
            && root.scrollHeight <= innerHeight && root.scrollWidth <= innerWidth;
        return still ? null : `${extent} in a ${innerWidth}x${innerHeight} viewport`;
    });
    if (moved) throw new Failure(`the document scrolls (${moved}); only beui's Scrolls may`);
    const ring = await page.evaluate(() => {
        const focused = document.activeElement;
        if (!focused?.closest(".beui-root") || !focused.hasAttribute("tabindex") || focused.style.outline) return null;
        const style = getComputedStyle(focused);
        return style.outlineStyle === "none" ? null : `${style.outlineStyle} ${style.outlineWidth}`;
    });
    if (ring) throw new Failure(`the browser draws its own focus ring (${ring}) beside beui's`);
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
            await pageChecks(opened.page);
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
