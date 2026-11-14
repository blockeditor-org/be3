// Push notifications on the whole web page (plans/notifications.md), with the real sw.js and
// push.js: Ada writes in a chat Dev shared with her while Dev's tab is closed, be-server pushes
// it to Dev's endpoint, and Dev's service worker shows it under the block's name and Ada's, as
// Dev's device names them, with what she wrote, opened with the block's key. Without a tab to
// open it, or without the key, it shows "New message"; a push delivered again shows nothing new;
// and signing out has the server forget the endpoint.
export default async function (test) {
    const ada = await test.account("Ada");

    // Dev and Ada become contacts through Dev's link.
    await test.drive("click", "#file-tree.host-menu");
    await test.drive("click", "#host.menu.contacts");
    await test.drive("click", "#host.contacts.new-link");
    await test.drive("wait", "#host.contacts.link");
    const link = (await test.drive("tree"))
        .split("\n")
        .map((line) => line.split('value="')[1]?.split('"')[0])
        .find((value) => value?.includes("#contact/"));
    await test.drive("key", "Escape");
    await test.drive("wait", "not", 'Dialog "Contacts"');
    await ada.drive("click", "#file-tree.host-menu");
    await ada.drive("click", "#host.menu.contacts");
    await ada.drive("click", "#host.contacts.paste");
    await ada.drive("type", link);
    await ada.drive("click", "#host.contacts.open");
    // The offer comes from the server.
    await ada.drive("wait", "#host.contacts.offered.accept");
    await ada.drive("click", "#host.contacts.offered.accept");
    await ada.drive("wait", 'Label value="Dev"');
    await ada.drive("key", "Escape");
    await ada.drive("wait", "not", 'Dialog "Contacts"');
    // Dev's device hears of Ada as a contact, which it names her by and shares with.
    await test.drive("click", "#file-tree.host-menu");
    await test.drive("click", "#host.menu.contacts");
    await test.drive("wait", 'Label value="Ada"');
    await test.drive("key", "Escape");
    await test.drive("wait", "not", 'Dialog "Contacts"');

    // Dev turns notifications on, which registers this browser's endpoint with the server.
    await test.drive("click", 'Tab "Settings"');
    await test.drive("wait", "#settings.push-device");
    await test.drive("click", "#settings.push-device");
    await test.drive("wait", 'Switch "Notifications on this device" toggled=True');
    const [endpoint] = await test.until(async () => {
        const held = await test.endpoints();
        return held.length > 0 && held;
    }, "the server holding Dev's endpoint");

    // Dev makes a chat, Roadmap, and shares it with Ada, who opens it.
    await test.drive("click", 'Tab "Files"');
    await test.drive("click", "#file-tree.new");
    await test.drive("click", 'MenuItem "New in My files…"');
    await test.drive("wait", "#picker.more");
    await test.drive("click", "#picker.more");
    await test.drive("wait", 'Button "Chat"');
    await test.drive("click", 'Button "Chat"');
    await test.drive("wait", 'TextInput "Name" value="Chat"');
    await test.drive("click", 'TextInput "Name" value="Chat"');
    await test.drive("key", "ctrl+a");
    await test.drive("type", "Roadmap");
    await test.drive("key", "Enter");
    await test.drive("wait", 'TreeItem "Roadmap"');
    await test.drive("wait", "#dock.editors.pin.editor.share");
    await test.drive("click", "#dock.editors.pin.editor.share");
    await test.drive("wait", "#share.query");
    await test.drive("click", "#share.query");
    await test.drive("type", "Ada");
    await test.drive("key", "Enter");
    await test.drive("wait", "#share.add");
    await test.drive("click", "#share.add");
    await test.drive("wait", 'Label value="ada@localhost"');
    await test.drive("click", "#share.close");
    // The share reaches Ada's files from the server.
    await ada.drive("wait", 'TreeItem "Roadmap"');
    await ada.drive("click", 'TreeItem "Roadmap"');
    await ada.drive("wait", "#chat.draft");

    // With Dev's tab closed, Dev is not connected, so what Ada writes is pushed to Dev.
    const says = async (text) => {
        await ada.drive("click", "#chat.draft");
        await ada.drive("type", text);
        await ada.drive("click", "#chat.send");
        return test.pushes.next();
    };
    await test.page.close();
    const lunch = await says("lunch?");
    const noon = await says("see you at noon");

    const expect = (condition, what, shown) => {
        if (!condition) throw new Error(`${what}: ${JSON.stringify(shown, null, 1)}`);
    };
    const generic = (notification) => notification.title === "New message" && notification.body === "";

    // No tab is open to open it with the block's key: it names nothing.
    await test.deliverPush(noon);
    let shown = await test.notifications();
    expect(shown.length === 1 && generic(shown[0]), "with no tab, a push shows New message", shown);

    // With a tab, the page opens it: the block's name, then Ada's and what she wrote.
    await test.openPage();
    await test.deliverPush(lunch);
    shown = await test.notifications();
    expect(shown.length === 2, "a push with a tab open shows one notification", shown);
    expect(shown[1].title === "Roadmap" && shown[1].body === "Ada: lunch?", "it names the block and its sender", shown);

    // The server may send one again: it shows nothing new.
    await test.deliverPush(lunch);
    shown = await test.notifications();
    expect(shown.length === 2, "the same push delivered again shows once", shown);

    // Sealed under a key this device does not hold: it names nothing.
    const foreign = Uint8Array.from(lunch, (byte, at) => (at < 48 ? byte : byte ^ 0x5a));
    await test.deliverPush(foreign);
    shown = await test.notifications();
    expect(shown.length === 3 && generic(shown[2]), "without the key, a push shows New message", shown);

    // Signing out has the server forget the endpoint, and drops the browser's subscription.
    await test.drive("click", "#file-tree.host-menu");
    await test.drive("click", "#host.menu.accounts");
    await test.drive("wait", "#account.options.Dev");
    await test.drive("click", "#account.options.Dev");
    await test.drive("wait", 'MenuItem "Log out"');
    await test.drive("click", 'MenuItem "Log out"');
    await test.drive("wait", 'Label value="No accounts yet. Add one to get started."');
    await test.until(
        async () => !(await test.endpoints()).some((held) => held.address === endpoint.address),
        "the server forgetting Dev's endpoint once Dev signed out",
    );
    await test.until(
        () =>
            test.page.evaluate(
                async () => !(await (await navigator.serviceWorker.ready).pushManager.getSubscription()),
            ),
        "the browser dropping its subscription once its last account signed out",
    );
}
