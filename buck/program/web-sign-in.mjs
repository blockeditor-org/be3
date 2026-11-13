// Signs the web host on a page into dev@localhost on the page's own be-server, as a person
// would through the portal, making the account the first time: what web-drive.mjs's
// dev-sign-in and web-host-test.mjs do. serverLog is the server's log, where a development
// server writes the link that verifies a new account's email, and invite an invite code
// (`be-server invite-code create`), which registering needs.

import { readFile } from "node:fs/promises";

const CODE = /https?:\/\/[^\s"]+\/auth\?code=[a-f0-9-]+/;

export async function signIn(page, serverLog, invite, timeout) {
    await page.waitForFunction(() => globalThis.blockStarted !== undefined, null, { timeout: timeout * 1000 });
    const tree = await page.evaluate(async (timeoutMs) => {
        await globalThis.blockStarted;
        const app = await import(new URL("be_host.js", document.baseURI).href);
        const deadline = performance.now() + timeoutMs;
        while (performance.now() < deadline) {
            const tree = await app.automate(["tree"], 1000);
            if (/https?:\/\/[^\s"]+\/auth\?code=[a-f0-9-]+/.test(tree) || tree.includes('"My files"')) return tree;
            await new Promise(requestAnimationFrame);
        }
        throw new Error("The development app did not show its sign-in code or workspace.");
    }, timeout * 1000);
    const url = tree.match(CODE)?.[0];
    if (!url) return;
    const portal = await page.context().newPage();
    try {
        await approve(portal, url, serverLog, invite);
    } catch (error) {
        const shown = await portal.locator("body").innerText().catch(() => "");
        throw new Error(`${error.message}\nThe portal at ${portal.url()} shows:\n${shown}`);
    }
    await portal.close();
}

async function approve(portal, url, serverLog, invite) {
    await portal.goto(url);
    const another = portal.getByText("Use another account", { exact: true });
    if (await another.count()) await another.click();
    await portal.getByLabel("Email", { exact: true }).fill("dev@localhost");
    await portal.getByLabel("Password", { exact: true }).fill("dev-password");
    await portal.getByRole("button", { name: "Sign in", exact: true }).click();
    const unknown = portal.getByText(/no account matches those credentials/i);
    await unknown.or(portal.getByRole("button", { name: "Approve sign-in", exact: true })).first().waitFor();
    if (await unknown.count()) {
        await portal.getByRole("link", { name: "Create an account", exact: true }).click();
        if (!invite) throw new Error("Registering needs an invite code, and none was given.");
        await portal.getByLabel("Invite code", { exact: true }).fill(invite);
        await portal.getByLabel("Display name", { exact: true }).fill("Dev");
        await portal.getByLabel("Email", { exact: true }).fill("dev@localhost");
        await portal.locator('[name="password"]').fill("dev-password");
        await portal.getByLabel("Confirm password", { exact: true }).fill("dev-password");
        await portal.getByRole("button", { name: "Create account", exact: true }).click();
        await portal.getByText("Check your email", { exact: true }).first().waitFor();
        const links = (await readFile(serverLog, "utf8")).match(/http:\/\/[^\s]+\/auth\/verify\?token=[a-f0-9]+/g);
        if (!links?.length) throw new Error("The development server did not log a verification link.");
        await portal.goto(links.at(-1));
        await portal.getByRole("button", { name: "Verify email", exact: true }).click();
    }
    await portal.getByRole("button", { name: "Approve sign-in", exact: true }).click();
    await portal.getByText("Sign-in approved", { exact: true }).first().waitFor();
}
