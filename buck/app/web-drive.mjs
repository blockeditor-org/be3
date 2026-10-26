// Drives the page web-dev.sh opened, one command per run:
// guides/running/web.md lists them.

import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const usage = `usage: drive [--timeout=SECONDS] [--no-settle] [--changes=N|all] [screen|app] COMMAND, or drive - < COMMANDS
The app answers the same commands as the native app's drive (help lists them),
such as app tree, app click TARGET, app type TEXT, app wait TEXT and settle. The
browser is the platform here, so it answers these screen commands itself:
  screen shot FILE [TARGET] save a screenshot, of TARGET only when given
  screen upload FILE TARGET click TARGET and answer the file chooser with FILE
and these of its own:
  eval EXPRESSION           evaluate JavaScript in the page and print the result
  reload                    reload the page
  size W H                  make the page W by H
  quit                      close the browser`;

let args = process.argv.slice(2);
let timeout = 30;
const options = [];
while (args[0]?.startsWith("--")) {
    if (args[0].startsWith("--timeout=")) {
        timeout = Number(args[0].slice("--timeout=".length));
    } else if (args[0] === "--no-settle" || args[0].startsWith("--changes=")) {
        options.push(args[0]);
    } else {
        break;
    }
    args = args.slice(1);
}
const [command, ...rest] = args;
const number = (index) => {
    const value = Number(rest[index]);
    if (!Number.isFinite(value)) {
        console.error(usage);
        process.exit(1);
    }
    return value;
};
if (!command) {
    console.error(usage);
    process.exit(1);
}

const endpoint = process.env.BLOCK_WEB_DEV;
if (!endpoint) {
    console.error("BLOCK_WEB_DEV is not set: source the env file web-dev printed.");
    process.exit(1);
}
const browser = await chromium.connectOverCDP(endpoint);
const page = browser.contexts()[0]?.pages().find((page) => page.url().startsWith("http"));
if (!page) {
    console.error("The browser has no page open: run web-dev again.");
    process.exit(1);
}

const automate = (words) =>
    page.evaluate(
        async ({ words, timeout }) => {
            const app = await import(new URL("block_app_lib.js", document.baseURI).href);
            try {
                return { text: await app.automate(words, Math.round(timeout * 1000)) };
            } catch (error) {
                return { error: String(error?.message ?? error) };
            }
        },
        { words, timeout },
    );
const answer = async (words) => {
    const { text, error } = await automate(words);
    if (error !== undefined) {
        console.error(error);
        process.exit(1);
    }
    return text;
};
const place = async (target) => {
    const reply = await answer(["app", "find", target]);
    const found = reply.match(/at (-?[\d.]+),(-?[\d.]+) size ([\d.]+)x([\d.]+)/);
    if (found === null) {
        console.error(`${target} has no place on screen: ${reply.trim()}`);
        process.exit(1);
    }
    const ratio = await page.evaluate(() => devicePixelRatio);
    const [x, y, width, height] = found.slice(1).map((value) => Number(value) / ratio);
    return { x, y, width, height };
};

async function run(command, rest) {
const number = (index) => {
    const value = Number(rest[index]);
    if (!Number.isFinite(value)) {
        console.error(usage);
        process.exit(1);
    }
    return value;
};
switch (command) {
    case "dev-sign-in": {
        const tree = await page.evaluate(async (timeoutMs) => {
            const app = await import(new URL("block_app_lib.js", document.baseURI).href);
            const deadline = performance.now() + timeoutMs;
            while (performance.now() < deadline) {
                const tree = await app.automate(["app", "tree"], 1000);
                if (/https?:\/\/[^\s"]+\/auth\?code=[a-f0-9-]+/.test(tree) || tree.includes('"My files"')) return tree;
                await new Promise(requestAnimationFrame);
            }
            throw new Error("The development app did not show its sign-in code or workspace.");
        }, timeout * 1000);
        const url = tree.match(/https?:\/\/[^\s"]+\/auth\?code=[a-f0-9-]+/)?.[0];
        if (!url) break;
        const portal = await page.context().newPage();
        await portal.goto(url);
        if (await portal.getByRole("button", { name: "Sign out / use another account", exact: true }).count()) {
            await portal.getByRole("button", { name: "Sign out / use another account", exact: true }).click();
        }
        await portal.getByLabel("Email", { exact: true }).fill("dev@localhost");
        await portal.getByLabel("Password", { exact: true }).fill("dev-password");
        await portal.getByRole("button", { name: "Sign in", exact: true }).click();
        await portal.getByRole("heading", { name: "Please check your submission" }).or(
            portal.getByRole("button", { name: "Approve sign-in", exact: true }),
        ).first().waitFor();
        if (await portal.getByRole("heading", { name: "Please check your submission" }).count()) {
            await portal.getByRole("link", { name: "Create an account", exact: true }).click();
            await portal.getByLabel("Display name", { exact: true }).fill("Dev");
            await portal.getByLabel("Email", { exact: true }).fill("dev@localhost");
            await portal.locator('[name="password"]').fill("dev-password");
            await portal.getByLabel("Confirm password", { exact: true }).fill("dev-password");
            await portal.getByRole("button", { name: "Create account", exact: true }).click();
            await portal.getByRole("heading", { name: "Check your email", exact: true }).waitFor();
            const { readFile } = await import("node:fs/promises");
            const log = await readFile(rest[0], "utf8");
            const links = log.match(/http:\/\/[^\s]+\/auth\/verify\?token=[a-f0-9]+/g);
            if (!links?.length) throw new Error("The development server did not log a verification link.");
            await portal.goto(links.at(-1));
            await portal.getByRole("button", { name: "Verify email", exact: true }).click();
        }
        await portal.getByRole("button", { name: "Approve sign-in", exact: true }).click();
        await portal.getByRole("heading", { name: "Sign-in approved", exact: true }).waitFor();
        await portal.close();
        break;
    }

    case "screen":
        return screen(rest[0], rest.slice(1));
    default:
        if (!(await browserCommand(command, rest, number))) {
            process.stdout.write(await answer([...options, command, ...rest]));
        }
}
}

async function screen(command, rest) {
switch (command) {
    case "shot":
        await page.screenshot({
            path: rest[0],
            clip: rest.length >= 2 ? await place(rest.slice(1).join(" ")) : undefined,
        });
        console.log(rest[0]);
        break;
    case "upload": {
        const { x, y, width, height } = await place(rest.slice(1).join(" "));
        const chosen = page.waitForEvent("filechooser");
        await page.mouse.click(x + width / 2, y + height / 2);
        await (await chosen).setFiles(rest[0]);
        break;
    }
    case "grab":
    case "drop":
    case "ungrab":
    case "dismiss":
        console.error(`${command} needs a native headless app; the browser has no way to send it`);
        process.exit(1);
        break;
    default:
        process.stdout.write(await answer([...options, "screen", command, ...rest]));
}
}

async function browserCommand(command, rest, number) {
switch (command) {
    case "eval":
        console.log(await page.evaluate(rest.join(" ")));
        break;
    case "reload":
        await page.reload();
        break;
    case "size": {
        const session = await page.context().newCDPSession(page);
        const { windowId } = await session.send("Browser.getWindowForTarget");
        const [outer, inner] = await page.evaluate(() => [
            [outerWidth, outerHeight],
            [innerWidth, innerHeight],
        ]);
        await session.send("Browser.setWindowBounds", {
            windowId,
            bounds: {
                width: number(0) + outer[0] - inner[0],
                height: number(1) + outer[1] - inner[1],
            },
        });
        break;
    }
    case "quit": {
        const session = await browser.newBrowserCDPSession();
        await session.send("Browser.close").catch(() => {});
        break;
    }
    default:
        return false;
}
return true;
}

const words = (line) => {
    const found = [];
    const pattern = /'([^']*)'|"((?:\\.|[^"\\])*)"|(\S+)/g;
    for (const match of line.matchAll(pattern)) {
        found.push(match[1] ?? match[2]?.replace(/\\(.)/g, "$1") ?? match[3]);
    }
    return found;
};

if (command === "-") {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(chunk);
    for (const line of Buffer.concat(chunks).toString().split("\n")) {
        const [first, ...others] = words(line);
        if (!first || first.startsWith("#")) continue;
        console.log(`> ${line.trim()}`);
        await run(first, others);
    }
} else {
    await run(command, rest);
}
process.exit(0);
