// Runs a test of the whole web host (web_host_test in drive.bzl) in Chromium through
// Playwright:
//
//   node web-host-test.mjs --bundle=DIR --server=BIN --caddy=DIR --caddyfile=FILE TEST.mjs
//
// It starts be-server and Caddy in front of the bundle in a directory of its own, as web-dev.sh
// does, opens the page in Chromium with `?dev-workspace&automation`, signs in as dev@localhost
// through the portal (web-sign-in.mjs), waits for the workspace, and calls TEST.mjs's default
// export with a `test` object, which is Dev's account (below) with:
//
//   await test.account(name)   another account, `name` (its address is name@localhost), signed
//                               in on a page of its own browser context, so of its own storage
//   test.serverLog             the server's log file
//   await test.endpoints()      the push endpoints the server holds, [{ account, address }]
//   await test.until(check, what)   check()'s first truthy answer, asked again until one comes:
//                               for state nothing announces, such as the server's database
//
// An account is { name, page, context, drive, openPage, deliverPush, notifications, pushes }:
//
//   page, context              its Playwright page and browser context
//   await drive(...words)      a drive command the host answers in the page, as web-drive.mjs
//                               sends it; it throws what the host says when it fails
//   await openPage()           opens the host on a new page of the context (after the test
//                               closed `page`, say) and waits for its workspace
//   await deliverPush(data)    has the context's service worker take a push whose payload is
//                               data (bytes or text), as a push service would deliver it, and
//                               resolves once the worker has handled it, with a tab or without
//   await notifications()      every notification the worker showed, in order
//   await pushes.next()        the payload of the next push the server posted to the account's
//                               endpoint, decrypted (RFC 8291) as the browser would
//
// The test stands in for the push service. Headless Chromium has none and never grants the
// permission to show notifications, so each page is given a subscription to an endpoint the
// test serves (Notification.permission, PushManager.subscribe and getSubscription), be-server
// is let push to it (--push-host, --allow-local-push-endpoints), and the worker's
// showNotification and getNotifications keep what it shows instead of showing it. sw.js and
// push.js are otherwise the real ones. A push is dispatched in the worker rather than through
// DevTools' ServiceWorker.deliverPushMessage, which carries text only and needs a page.
//
// The test fails by throwing. The browser is BE3_CHROME when set, else Playwright's Chromium.
// It draws with WebGL on SwiftShader, as `web-dev --webgl` does.

import { execFileSync, spawn } from "node:child_process";
import { createDecipheriv, createECDH, createHmac, randomBytes } from "node:crypto";
import { createWriteStream } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer as createHttpServer } from "node:http";
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

// Waits until `check()` resolves to something truthy, which it returns: for state outside the
// page that nothing announces, such as the server's database.
async function until(check, what, seconds = TIMEOUT) {
    const deadline = Date.now() + seconds * 1000;
    for (;;) {
        const found = await check();
        if (found) return found;
        if (Date.now() > deadline) throw new Error(`${what} did not happen within ${seconds} s`);
        await new Promise((resolve) => setTimeout(resolve, 100));
    }
}

// A queue of values, each taken once by `next()`.
function queue() {
    const held = [];
    const waiting = [];
    return {
        push(value) {
            const taker = waiting.shift();
            if (taker) taker(value);
            else held.push(value);
        },
        next() {
            return held.length ? Promise.resolve(held.shift()) : new Promise((resolve) => waiting.push(resolve));
        },
    };
}

// The push service the test stands in for: endpoints it serves, and the keys of the one
// subscription every page is given, which it decrypts what the server posts with
// (RFC 8291's aes128gcm).
const receiver = createECDH("prime256v1");
receiver.generateKeys();
const pushAuth = randomBytes(16);

function hmac(key, ...data) {
    const mac = createHmac("sha256", key);
    for (const part of data) mac.update(part);
    return mac.digest();
}

function decryptPush(body) {
    const salt = body.subarray(0, 16);
    const idLength = body[20];
    const sender = body.subarray(21, 21 + idLength);
    const encrypted = body.subarray(21 + idLength);
    const shared = receiver.computeSecret(sender);
    const keyInfo = Buffer.concat([Buffer.from("WebPush: info\0"), receiver.getPublicKey(), sender]);
    const ikm = hmac(hmac(pushAuth, shared), keyInfo, Buffer.from([1]));
    const prk = hmac(salt, ikm);
    const key = hmac(prk, Buffer.from("Content-Encoding: aes128gcm\0"), Buffer.from([1])).subarray(0, 16);
    const nonce = hmac(prk, Buffer.from("Content-Encoding: nonce\0"), Buffer.from([1])).subarray(0, 12);
    const decipher = createDecipheriv("aes-128-gcm", key, nonce);
    decipher.setAuthTag(encrypted.subarray(encrypted.length - 16));
    const padded = Buffer.concat([decipher.update(encrypted.subarray(0, encrypted.length - 16)), decipher.final()]);
    // The last record ends in a 2, then any zeros of padding.
    let end = padded.length;
    while (end > 0 && padded[end - 1] === 0) end--;
    if (padded[end - 1] !== 2) throw new Error("a push's padding is not the last record's");
    return new Uint8Array(padded.subarray(0, end - 1));
}

const endpoints = new Map();
function endpointQueue(path) {
    if (!endpoints.has(path)) endpoints.set(path, queue());
    return endpoints.get(path);
}
const pushServer = createHttpServer((request, response) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
        try {
            endpointQueue(request.url).push(decryptPush(Buffer.concat(chunks)));
            response.statusCode = 201;
        } catch (error) {
            console.error(`a push to ${request.url} could not be decrypted: ${error}`);
            response.statusCode = 400;
        }
        response.end();
    });
});

// Gives a page the subscription the test serves. It runs in the page, before its scripts.
function standInPushService({ address, p256dh, auth }) {
    const STORED = "be3-test-push-subscription";
    const bytes = (base64) => Uint8Array.from(atob(base64), (letter) => letter.charCodeAt(0));
    const base64 = (array) => btoa(String.fromCharCode(...array));
    Object.defineProperty(Notification, "permission", { configurable: true, get: () => "granted" });
    Notification.requestPermission = async () => "granted";
    const subscription = (serverKey) => ({
        endpoint: address,
        expirationTime: null,
        options: { userVisibleOnly: true, applicationServerKey: bytes(serverKey).buffer },
        getKey: (name) => bytes(name === "p256dh" ? p256dh : auth).buffer,
        async unsubscribe() {
            localStorage.removeItem(STORED);
            return true;
        },
    });
    PushManager.prototype.subscribe = async function ({ applicationServerKey }) {
        const key = ArrayBuffer.isView(applicationServerKey)
            ? new Uint8Array(applicationServerKey.buffer, applicationServerKey.byteOffset, applicationServerKey.byteLength)
            : new Uint8Array(applicationServerKey);
        localStorage.setItem(STORED, base64(key));
        return subscription(base64(key));
    };
    PushManager.prototype.getSubscription = async function () {
        const held = localStorage.getItem(STORED);
        return held === null ? null : subscription(held);
    };
}

// Keeps what the worker shows instead of showing it. It runs in the worker.
function standInNotifications() {
    if (self.beTestNotifications) return;
    const shown = [];
    let onScreen = [];
    self.beTestNotifications = shown;
    self.registration.showNotification = async (title, options = {}) => {
        const notification = {
            title,
            body: options.body ?? "",
            tag: options.tag ?? "",
            renotify: options.renotify ?? false,
            data: options.data ?? null,
        };
        shown.push(notification);
        onScreen = onScreen.filter((held) => !held.tag || held.tag !== notification.tag);
        onScreen.push(notification);
    };
    self.registration.getNotifications = async ({ tag } = {}) =>
        onScreen
            .filter((held) => tag === undefined || held.tag === tag)
            .map((held) => ({
                ...held,
                close() {
                    onScreen = onScreen.filter((other) => other !== held);
                },
            }));
}

// Hands `data` to the worker's push listeners as the browser would, and resolves once what they
// gave waitUntil settles. It runs in the worker.
async function dispatchPush(data) {
    const waited = [];
    const event = new PushEvent("push", typeof data === "string" ? { data } : { data: new Uint8Array(data) });
    Object.defineProperty(event, "waitUntil", { value: (promise) => waited.push(promise) });
    self.dispatchEvent(event);
    await Promise.all(waited);
}

// Chromium's singleton socket lives in TMPDIR, and buck's is too long a path for a Unix socket.
const directory = await mkdtemp("/tmp/be3-web-host-test-");
const serverLog = join(directory, "server.log");
const serverData = join(directory, "server");
const children = [];
const start = (program, args, env, log) => {
    const child = spawn(program, args, { env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] });
    const out = createWriteStream(log);
    child.stdout.pipe(out);
    child.stderr.pipe(out);
    children.push(child);
    return child;
};

const [port, backend, admin, push] = [await freePort(), await freePort(), await freePort(), await freePort()];
const origin = `http://127.0.0.1:${port}`;
let failed = true;
let browser;
try {
    start(
        server,
        [
            "--build-info-file", join(bundle, "build-info.json"),
            "--addr", `127.0.0.1:${backend}`,
            "--data-dir", serverData,
            "--allow-login", "--allow-registration",
            "--auth-origin", origin,
            "--push-host", "127.0.0.1",
            "--allow-local-push-endpoints",
        ],
        {},
        serverLog,
    );
    start(
        caddy,
        ["run", "--adapter", "caddyfile", "--config", caddyfile],
        {
            BE3_SITE: origin,
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
    await new Promise((done) => pushServer.listen(push, "127.0.0.1", done));
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

    const account = async ({ name, email }) => {
        const slug = name.toLowerCase();
        const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
        await context.addInitScript(standInPushService, {
            address: `http://127.0.0.1:${push}/push/${slug}`,
            p256dh: receiver.getPublicKey().toString("base64"),
            auth: pushAuth.toString("base64"),
        });
        // The worker that registered last, its stand-in notifications installed.
        let worker;
        context.on("serviceworker", (started) => {
            worker = started.evaluate(standInNotifications).then(() => started);
            worker.catch(() => {});
        });
        const opened = {
            name,
            context,
            pushes: endpointQueue(`/push/${slug}`),
        };
        const watch = (page) => {
            page.on("console", (message) => console.log(`[${slug} ${message.type()}] ${message.text()}`));
            page.on("pageerror", (error) => console.log(`[${slug} error] ${error}`));
        };
        opened.drive = async (...words) => {
            const { text, error } = await opened.page.evaluate(
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
                const shown = words[0] === "tree" ? "" : await opened.drive("--no-settle", "tree").catch(() => "");
                throw new Error(`${name}: drive ${words.join(" ")}: ${error}\nWhat the host shows:\n${shown}`);
            }
            return text;
        };
        // The host takes its window's size from a resize, as web-dev's `drive size` gives it.
        const ready = async () => {
            await opened.page.setViewportSize({ width: 1279, height: 800 });
            await opened.page.setViewportSize({ width: 1280, height: 800 });
            await opened.page.waitForFunction(() => globalThis.blockStarted !== undefined, null, {
                timeout: TIMEOUT * 1000,
            });
            await opened.drive("settle");
            await opened.drive("wait", 'TreeItem "My files"');
            await opened.drive("settle");
        };
        opened.openPage = async () => {
            opened.page = await context.newPage();
            watch(opened.page);
            await opened.page.goto(`${origin}/?automation`);
            await ready();
        };
        opened.deliverPush = async (data) => {
            if (!worker) throw new Error(`${name}: no service worker is registered`);
            const bytes = typeof data === "string" ? data : Array.from(data);
            await (await worker).evaluate(dispatchPush, bytes);
        };
        opened.notifications = async () => {
            if (!worker) return [];
            return (await worker).evaluate(() => self.beTestNotifications);
        };

        opened.page = await context.newPage();
        watch(opened.page);
        const invite = execFileSync(server, ["invite-code", "create", "--data-dir", serverData], {
            encoding: "utf8",
        }).trim();
        await opened.page.goto(`${origin}/?dev-workspace&automation`);
        await signIn(opened.page, serverLog, invite, TIMEOUT, { name, email });
        await ready();
        return opened;
    };

    const { DatabaseSync } = createRequire(import.meta.url)("node:sqlite");
    const test = await account({ name: "Dev", email: "dev@localhost" });
    Object.assign(test, {
        account: (name) => account({ name, email: `${name.toLowerCase()}@localhost` }),
        serverLog,
        endpoints: async () => {
            const database = new DatabaseSync(join(serverData, "metadata.sqlite"), { readOnly: true });
            try {
                return database
                    .prepare("SELECT account_id AS account, address FROM push_endpoints")
                    .all()
                    .map((row) => ({ ...row }));
            } finally {
                database.close();
            }
        },
        until,
    });
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
    pushServer.close();
    for (const child of children) child.kill();
    await rm(directory, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
