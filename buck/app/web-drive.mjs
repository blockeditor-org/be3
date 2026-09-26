// Drives the page web-dev.sh opened, one command per run:
// guides/running_the_web_app.md lists them.

import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const usage = `usage: drive COMMAND
  tree                      print the accessibility tree
  shot FILE [X Y W H]       save a screenshot, or the region at X,Y of W by H
  click X Y [right]         click at X,Y
  dblclick X Y              double-click at X,Y
  move X Y                  move the pointer to X,Y
  drag X1 Y1 X2 Y2          drag with the left button from X1,Y1 to X2,Y2
  wheel X Y DY              scroll by DY at X,Y, down when positive
  type TEXT...              type the text
  key KEY                   press a key, such as Enter or Control+z
  upload FILE X Y           click at X,Y and answer the file chooser with FILE
  eval EXPRESSION           evaluate JavaScript in the page and print the result
  reload                    reload the page
  size W H                  make the page W by H
  quit                      close the browser`;

const [command, ...rest] = process.argv.slice(2);
const number = (index) => {
    const value = Number(rest[index]);
    if (!Number.isFinite(value)) {
        console.error(usage);
        process.exit(1);
    }
    return value;
};

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

switch (command) {
    case "tree":
        process.stdout.write(
            (await page.evaluate(async () => {
                const app = await import(new URL("block_app_lib.js", location.href).href);
                return app.accessibility_tree() ?? "";
            })) ?? "",
        );
        break;
    case "shot":
        await page.screenshot({
            path: rest[0],
            clip:
                rest.length >= 5
                    ? { x: number(1), y: number(2), width: number(3), height: number(4) }
                    : undefined,
        });
        break;
    case "click":
        await page.mouse.click(number(0), number(1), {
            button: rest[2] === "right" ? "right" : "left",
        });
        break;
    case "dblclick":
        await page.mouse.dblclick(number(0), number(1));
        break;
    case "move":
        await page.mouse.move(number(0), number(1));
        break;
    case "drag":
        await page.mouse.move(number(0), number(1));
        await page.mouse.down();
        await page.mouse.move(number(2), number(3), { steps: 10 });
        await page.mouse.up();
        break;
    case "wheel":
        await page.mouse.move(number(0), number(1));
        await page.mouse.wheel(0, number(2));
        break;
    case "type":
        await page.keyboard.type(rest.join(" "), { delay: 20 });
        break;
    case "key":
        await page.keyboard.press(rest[0]);
        break;
    case "upload": {
        const chosen = page.waitForEvent("filechooser");
        await page.mouse.click(number(1), number(2));
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
    default:
        console.error(usage);
        process.exit(1);
}
process.exit(0);
