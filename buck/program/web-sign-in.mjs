// Signs the web host on a page into an account on the page's own be-server, as a person
// would through the portal, making the account the first time: what web-drive.mjs's
// dev-sign-in and web-host-test.mjs do. serverLog is the server's log, where a development
// server writes the codes it emails, and invite an invite code
// (`be-server invite-code create`), which registering needs. The account is dev@localhost,
// named Dev, unless `account` names another ({ name, email }).

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
    // The portal emails a code, which a development server writes to its log. An address with
    // no account is sent none, so the account is made: registering emails a code too.
    let code = await emailedCode(serverLog, email, async () => {
        await portal.getByLabel("Email", { exact: true }).fill(email);
        await portal.getByRole("button", { name: "Email me a code", exact: true }).click();
        await portal.getByText("Enter your code", { exact: true }).first().waitFor();
    });
    if (!code) {
        if (!invite) throw new Error("Registering needs an invite code, and none was given.");
        await portal.goto(url);
        await portal.getByRole("link", { name: "Create an account", exact: true }).click();
        code = await emailedCode(serverLog, email, async () => {
            // Filled once: typing that lands before the page hydrates must still reach it.
            const fields = [
                [portal.getByLabel("Invite code", { exact: true }), invite],
                [portal.getByLabel("Display name", { exact: true }), name],
                [portal.getByLabel("Email", { exact: true }), email],
            ];
            for (const [field, value] of fields) await field.fill(value);
            for (const [field, value] of fields) {
                const held = await field.inputValue();
                if (held !== value) throw new Error(`a registration field holds ${JSON.stringify(held)}, not what was typed`);
            }
            await portal.getByRole("button", { name: "Email me a code", exact: true }).click();
            await portal.getByText("Enter your code", { exact: true }).first().waitFor();
        });
        if (!code) throw new Error("The development server did not log a registration code.");
    }
    await portal.getByLabel("Code", { exact: true }).fill(code);
    await portal.getByRole("button", { name: "Continue", exact: true }).click();
    await portal.getByRole("button", { name: "Approve sign-in", exact: true }).click();
    await portal.getByText("Sign-in approved", { exact: true }).first().waitFor();
}

// The code the server logged for `email` while `ask` ran, or none: the server writes the
// message before it answers the request that asked for it.
async function emailedCode(serverLog, email, ask) {
    const codes = async () =>
        (await readFile(serverLog, "utf8"))
            .split("\n")
            .filter((line) => line.includes(email))
            .flatMap((line) => [...line.matchAll(/Your code is ([A-Za-z0-9-]+)\./g)].map((found) => found[1]));
    const before = (await codes()).length;
    await ask();
    return (await codes()).slice(before).at(-1);
}
