// Drives the page web-dev.sh opened, one command per run:
// guides/running/web.md lists them.

import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const usage = `usage: drive [--timeout=SECONDS] COMMAND
The app answers the same commands as the native app's drive (help lists them),
such as tree, click TARGET, type TEXT, key CHORD, wait TEXT and settle. These
are the browser's own:
  shot FILE [TARGET]        save a screenshot, of TARGET only when given
  upload FILE TARGET        click TARGET and answer the file chooser with FILE
  eval EXPRESSION           evaluate JavaScript in the page and print the result
  reload                    reload the page
  size W H                  make the page W by H
  quit                      close the browser`;

let args = process.argv.slice(2);
let timeout = 30;
if (args[0]?.startsWith("--timeout=")) {
    timeout = Number(args[0].slice("--timeout=".length));
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
            const app = await import(new URL("block_app_lib.js", location.href).href);
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
    const found = (await answer(["find", target])).match(/at (-?[\d.]+),(-?[\d.]+) size ([\d.]+)x([\d.]+)/);
    const ratio = await page.evaluate(() => devicePixelRatio);
    const [x, y, width, height] = found.slice(1).map((value) => Number(value) / ratio);
    return { x, y, width, height };
};

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
    case "drop":
    case "dismiss":
        console.error(`${command} needs a native headless app; the browser has no way to send it`);
        process.exit(1);
        break;
    default:
        process.stdout.write(await answer([command, ...rest]));
}
process.exit(0);
