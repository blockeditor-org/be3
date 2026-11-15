// Signs the web host on a page into an account on the page's own be-server, as a person
// would through the portal, making the account the first time: what web-drive.mjs's
// dev-sign-in and web-host-test.mjs do. serverLog is the server's log, where a development
// server writes the code it mails, and invite an invite code (`be-server invite-code
// create`), which registering needs: with one, it registers first, and signs in instead when
// the email already has an account. The account is dev@localhost, named Dev, unless
// `account` names another ({ name, email }).

import { readFile } from "node:fs/promises";

const CODE = /https?:\/\/[^\s"]+\/auth\?code=[a-f0-9-]+/;

const DEV = { name: "Dev", email: "dev@localhost" };

export async function signIn(page, serverLog, invite, timeout, account = DEV) {
    await page.waitForFunction(() => globalThis.blockStarted !== undefined, null, { timeout: timeout * 1000 });
    const tree = await page.evaluate(async (timeoutMs) => {
        await globalThis.blockStarted;
        const app = await import(new URL("be_host.js", document.baseURI).href);
        const deadline = performance.now() + timeoutMs;
        while (performance.now() < deadline) {
            // A tree can time out while the page is still starting: ask again.
            const tree = await app.automate(["tree"], 1000).catch(() => "");
            if (/https?:\/\/[^\s"]+\/auth\?code=[a-f0-9-]+/.test(tree) || tree.includes('"My files"')) return tree;
            await new Promise(requestAnimationFrame);
        }
        throw new Error("The development app did not show its sign-in code or workspace.");
    }, timeout * 1000);
    const url = tree.match(CODE)?.[0];
    if (!url) return;
    const portal = await page.context().newPage();
    try {
        await approve(portal, url, serverLog, invite, account);
    } catch (error) {
        const shown = await portal.locator("body").innerText().catch(() => "");
        throw new Error(`${error.message}\nThe portal at ${portal.url()} shows:\n${shown}`);
    }
    await portal.close();
}

async function approve(portal, url, serverLog, invite, { name, email }) {
    await portal.goto(url);
    const another = portal.getByText("Use another account", { exact: true });
    if (await another.count()) await another.click();
    const entering = portal.getByRole("button", { name: "Continue", exact: true });
    let mailed = await codes(serverLog, email);
    let registered = false;
    if (invite) {
        await portal.getByRole("link", { name: "Create an account", exact: true }).click();
        await fill(portal, [
            [portal.getByLabel("Invite code", { exact: true }), invite],
            [portal.getByLabel("Display name", { exact: true }), name],
            [portal.getByLabel("Email", { exact: true }), email],
        ]);
        await portal.getByRole("button", { name: "Email me a code", exact: true }).click();
        const taken = portal.getByText(/already registered/i);
        await taken.or(entering).first().waitFor();
        registered = (await taken.count()) === 0;
        if (!registered) await portal.goto(url);
    }
    if (!registered) {
        await fill(portal, [[portal.getByLabel("Email", { exact: true }), email]]);
        await portal.getByRole("button", { name: "Email me a code", exact: true }).click();
        await entering.waitFor();
    }
    const code = await newCode(serverLog, email, mailed.length);
    await fill(portal, [[portal.getByLabel("Code", { exact: true }), code]]);
    await entering.click();
    await portal.getByRole("button", { name: "Approve sign-in", exact: true }).click();
    await portal.getByText("Sign-in approved", { exact: true }).first().waitFor();
}

// Fills each field, then checks it holds what was typed: typing that lands before the page
// hydrates must still reach it.
async function fill(portal, fields) {
    for (const [field, value] of fields) await field.fill(value);
    for (const [field, value] of fields) {
        const held = await field.inputValue();
        if (held !== value) throw new Error(`a field holds ${JSON.stringify(held)}, not what was typed`);
    }
}

// Every code the server has logged mailing to `email`, oldest first.
async function codes(serverLog, email) {
    const log = await readFile(serverLog, "utf8").catch(() => "");
    return log
        .split("\n")
        .filter((line) => line.includes(email))
        .flatMap((line) => [...line.matchAll(/Your code is ([A-Z0-9]+)\./g)].map((found) => found[1]));
}

// The code the server logged for `email` after the first `before`, once the log has it.
async function newCode(serverLog, email, before) {
    const deadline = Date.now() + 10_000;
    while (Date.now() < deadline) {
        const mailed = await codes(serverLog, email);
        if (mailed.length > before) return mailed.at(-1);
        await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error(`The development server did not log a code for ${email}.`);
}
