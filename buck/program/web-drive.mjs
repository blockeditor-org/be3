// Drives the page web-dev.sh opened, one command per run:
// guides/running/web.md lists them.

import { createRequire } from "node:module";

import { Failure, commands, leadingOptions, shownPlaces } from "./drive-words.mjs";
import { signIn } from "./web-sign-in.mjs";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const usage = `usage: drive [--timeout=SECONDS] [--no-settle] [--changes=N|all] COMMAND, or drive - < COMMANDS
The app answers the same commands as the native app's drive (help lists them),
such as tree, click TARGET, type TEXT, wait TEXT and settle. The browser is the
platform here, so it answers these of the platform's commands itself:
  shot FILE [TARGET]        save a screenshot, of TARGET only when given
  upload FILE TARGET        click TARGET and answer the file chooser with FILE
and these of its own:
  eval EXPRESSION           evaluate JavaScript in the page and print the result
  reload                    reload the page and wait until the app answers
  size W H                  make the page W by H
  quit                      close the browser`;

const { timeout, options, rest: args } = leadingOptions(process.argv.slice(2));
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

const started = () =>
    page.waitForFunction(() => globalThis.blockStarted !== undefined, null, { timeout: timeout * 1000 });
const automate = async (words) => {
    await started();
    return page.evaluate(
        async ({ words, timeout }) => {
            try {
                await globalThis.blockStarted;
            } catch (error) {
                return { error: `the app did not start: ${error?.message ?? error}` };
            }
            const app = await import(new URL("be_host.js", document.baseURI).href);
            try {
                return { text: await app.automate(words, Math.round(timeout * 1000)) };
            } catch (error) {
                return { error: String(error?.message ?? error) };
            }
        },
        { words, timeout },
    );
};
const answer = async (words) => {
    const { text, error } = await automate(words);
    if (error !== undefined) {
        console.error(error);
        process.exit(1);
    }
    return text;
};
// Where target shows, once a scroll that cuts it has revealed it, as the
// app's own input commands aim.
const place = async (target) => {
    const ratio = await page.evaluate(() => devicePixelRatio);
    try {
        const [found] = await shownPlaces((command, target) => answer([command, target]), [target], ratio);
        return found;
    } catch (error) {
        if (!(error instanceof Failure)) throw error;
        console.error(error.message);
        process.exit(1);
    }
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
    case "dev-sign-in":
        await signIn(page, rest[0], timeout);
        break;

    default:
        if (!(await platform(command, rest)) && !(await browserCommand(command, rest, number))) {
            process.stdout.write(await answer([...options, command, ...rest]));
        }
}
}

async function platform(command, rest) {
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
        return false;
}
return true;
}

async function browserCommand(command, rest, number) {
switch (command) {
    case "eval":
        console.log(await page.evaluate(rest.join(" ")));
        break;
    case "reload":
        await page.reload();
        await answer(["--no-settle", "tree"]);
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

if (command === "-") {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(chunk);
    for (const { line, words } of commands(Buffer.concat(chunks).toString())) {
        const [first, ...others] = words;
        console.log(`> ${line}`);
        await run(first, others);
    }
} else {
    await run(command, rest);
}
process.exit(0);
