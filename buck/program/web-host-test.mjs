// Runs a test of the whole web host (web_host_test in drive.bzl) in Chromium through
// Playwright:
//
//   node web-host-test.mjs --bundle=DIR --server=BIN --caddy=DIR --caddyfile=FILE TEST.mjs
//
// It starts be-server and Caddy in front of the bundle in a directory of its own, as web-dev.sh
// does, opens the page in Chromium with `?dev-workspace&automation`, signs in as dev@localhost
// through the portal (web-sign-in.mjs), waits for the workspace, and calls TEST.mjs's default
// export with a `test` object:
//
//   test.page, test.context    the Playwright page and its browser context
//   await test.drive(...words)  a drive command the host answers in the page, as web-drive.mjs
//                               sends it; it throws what the host says when it fails
//   await test.deliverPush(text)   delivers a push whose payload is text to the page's service
//                               worker, as a push service would
//   test.serverLog             the server's log file
//
// The test fails by throwing. The browser is BE3_CHROME when set, else Playwright's Chromium.
// It draws with WebGL on SwiftShader, as `web-dev --webgl` does.

import { execFileSync, spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { signIn } from "./web-sign-in.mjs";

const TIMEOUT = 120;
const option = (name) => process.argv.find((argument) => argument.startsWith(`--${name}=`))?.slice(name.length + 3);
const bundle = resolve(option("bundle"));
const server = resolve(option("server"));
let caddy = resolve(option("caddy"));
const caddyfile = resolve(option("caddyfile"));
const script = process.argv.slice(2).find((argument) => !argument.startsWith("--"));
if (!bundle || !server || !caddy || !caddyfile || !script) {
    console.error("usage: web-host-test.mjs --bundle=DIR --server=BIN --caddy=DIR --caddyfile=FILE TEST.mjs");
    process.exit(2);
}
caddy = join(caddy, "caddy");

function playwright() {
    const require = createRequire(import.meta.url);
    try {
        return require("playwright");
    } catch {
        console.error("Playwright is missing: NODE_PATH names its modules on a worker.");
        process.exit(1);
    }
}

// A port nothing listens on yet.
function freePort() {
    return new Promise((done, failed) => {
        const probe = createServer();
        probe.once("error", failed);
        probe.listen(0, "127.0.0.1", () => {
            const { port } = probe.address();
            probe.close(() => done(port));
        });
    });
}

async function served(url, seconds) {
    const deadline = Date.now() + seconds * 1000;
    while (Date.now() < deadline) {
        try {
            if ((await fetch(url)).ok) return;
        } catch {}
        await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error(`${url} was not served within ${seconds} s`);
}

// Chromium's singleton socket lives in TMPDIR, and buck's is too long a path for a Unix socket.
const directory = await mkdtemp("/tmp/be3-web-host-test-");
const serverLog = join(directory, "server.log");
const children = [];
const start = (program, args, env, log) => {
    const child = spawn(program, args, { env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] });
    const out = createWriteStream(log);
    child.stdout.pipe(out);
    child.stderr.pipe(out);
    children.push(child);
    return child;
};

const [port, backend, admin] = [await freePort(), await freePort(), await freePort()];
const origin = `http://127.0.0.1:${port}`;
let failed = true;
let browser;
try {
    start(
        server,
        [
            "--build-info-file", join(bundle, "build-info.json"),
            "--addr", `127.0.0.1:${backend}`,
            "--data-dir", join(directory, "server"),
            "--allow-login", "--allow-registration",
            "--auth-origin", origin,
        ],
        {},
        serverLog,
    );
    start(
        caddy,
        ["run", "--adapter", "caddyfile", "--config", caddyfile],
        {
            BE3_SITE: `http://:${port}`,
            BE3_BIND: "127.0.0.1",
            BE3_BACKEND_URL: `127.0.0.1:${backend}`,
            BE3_WEB_ROOT: bundle,
            CADDY_ADMIN: `127.0.0.1:${admin}`,
            HOME: directory,
            XDG_DATA_HOME: directory,
            XDG_CONFIG_HOME: directory,
        },
        join(directory, "caddy.log"),
    );
    await served(`${origin}/`, 30);

    const { chromium } = playwright();
    browser = await chromium.launch({
        executablePath: process.env.BE3_CHROME || chromium.executablePath(),
        args: [
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-features=WebGPU",
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
            "--ignore-gpu-blocklist",
        ],
        env: { ...process.env, TMPDIR: directory },
    });
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const page = await context.newPage();
    page.on("console", (message) => console.log(`[page ${message.type()}] ${message.text()}`));
    page.on("pageerror", (error) => console.log(`[page error] ${error}`));

    const session = await context.newCDPSession(page);
    let registration;
    session.on("ServiceWorker.workerRegistrationUpdated", ({ registrations }) => {
        registration = registrations.at(-1)?.registrationId ?? registration;
    });
    await session.send("ServiceWorker.enable");

    const invite = execFileSync(server, ["invite-code", "create", "--data-dir", join(directory, "server")], {
        encoding: "utf8",
    }).trim();
    await page.goto(`${origin}/?dev-workspace&automation`);
    await signIn(page, serverLog, invite, TIMEOUT);

    const drive = async (...words) => {
        const { text, error } = await page.evaluate(
            async ({ words, timeout }) => {
                await globalThis.blockStarted;
                const app = await import(new URL("be_host.js", document.baseURI).href);
                try {
                    return { text: await app.automate(words, timeout) };
                } catch (error) {
                    return { error: String(error?.message ?? error) };
                }
            },
            { words, timeout: TIMEOUT * 1000 },
        );
        if (error !== undefined) {
            const shown = words[0] === "tree" ? "" : await drive("--no-settle", "tree").catch(() => "");
            throw new Error(`drive ${words.join(" ")}: ${error}\nWhat the host shows:\n${shown}`);
        }
        return text;
    };
    // The host takes its window's size from a resize, as web-dev's `drive size` gives it.
    await page.setViewportSize({ width: 1279, height: 800 });
    await page.setViewportSize({ width: 1280, height: 800 });
    await drive("settle");
    await drive("wait", 'TreeItem "My files"');
    await drive("settle");

    const test = {
        page,
        context,
        drive,
        serverLog,
        deliverPush: async (data) => {
            if (!registration) throw new Error("no service worker is registered");
            await session.send("ServiceWorker.deliverPushMessage", { origin, registrationId: registration, data });
        },
    };
    const run = (await import(pathToFileURL(resolve(script)).href)).default;
    await run(test);
    console.log(`${script} passed`);
    failed = false;
} catch (error) {
    console.error(`${script} failed: ${error?.stack ?? error}`);
    for (const log of ["server.log", "caddy.log"]) {
        try {
            const text = await readFile(join(directory, log), "utf8");
            console.error(`--- the end of ${log}\n${text.split("\n").slice(-40).join("\n")}`);
        } catch {}
    }
} finally {
    await browser?.close();
    for (const child of children) child.kill();
    await rm(directory, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
