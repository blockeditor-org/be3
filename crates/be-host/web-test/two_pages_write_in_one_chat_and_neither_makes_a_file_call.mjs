// The page shares no memory with the host's worker (plans/web_page_never_waits.md): two pages,
// Dev's and Ada's, each with its own browser context and host's worker, share a chat and each
// sees what the other writes in it, which travels page to worker to server and back by
// messages alone. Neither page makes a file call as it starts or runs, and one made on the page
// fails at once, naming the call, rather than spinning.
export default async function (test) {
    const refusal = "the page made a file call";
    const refused = [];
    const watch = (page) =>
        page.on("console", (message) => {
            if (message.text().includes(refusal)) refused.push(message.text());
        });
    const expect = (condition, what, shown) => {
        if (!condition) throw new Error(`${what}: ${JSON.stringify(shown, null, 1)}`);
    };

    // Dev's page opened again, watched from its start.
    test.context.on("page", watch);
    await test.page.close();
    await test.openPage();

    const ada = await test.account("Ada");
    watch(ada.page);

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
    await ada.drive("wait", "#host.contacts.offered.accept");
    await ada.drive("click", "#host.contacts.offered.accept");
    await ada.drive("wait", 'Label value="Dev"');
    await ada.drive("key", "Escape");
    await ada.drive("wait", "not", 'Dialog "Contacts"');
    await test.drive("click", "#file-tree.host-menu");
    await test.drive("click", "#host.menu.contacts");
    await test.drive("wait", 'Label value="Ada"');
    await test.drive("key", "Escape");
    await test.drive("wait", "not", 'Dialog "Contacts"');

    // Dev makes a chat, Plans, and shares it with Ada, who opens it.
    await test.drive("click", "#file-tree.new");
    await test.drive("click", 'MenuItem "New in My files…"');
    await test.drive("wait", "#picker.more");
    await test.drive("click", "#picker.more");
    await test.drive("wait", 'Button "Chat"');
    await test.drive("click", 'Button "Chat"');
    await test.drive("wait", 'TextInput "Name" value="Chat"');
    await test.drive("click", 'TextInput "Name" value="Chat"');
    await test.drive("key", "ctrl+a");
    await test.drive("type", "Plans");
    await test.drive("key", "Enter");
    await test.drive("wait", 'TreeItem "Plans"');
    await test.drive("wait", "#dock.editors.pin.editor.share");
    await test.drive("click", "#dock.editors.pin.editor.share");
    await test.drive("wait", "#share.query");
    await test.drive("click", "#share.query");
    await test.drive("wait", 'TextInput "Add contacts" focused');
    await test.drive("type", "Ada");
    await test.drive("wait", 'Button "Ada (ada@localhost)"');
    await test.drive("click", 'Button "Ada (ada@localhost)"');
    await test.drive("wait", "#share.add");
    await test.drive("click", "#share.add");
    await test.drive("wait", 'Label value="ada@localhost"');
    await test.drive("click", "#share.close");
    await ada.drive("wait", 'TreeItem "Plans"');
    await ada.drive("click", 'TreeItem "Plans"');
    await ada.drive("wait", "#chat.draft");
    await test.drive("wait", "#chat.draft");

    // Each writes, and each sees what the other wrote.
    await ada.drive("click", "#chat.draft");
    await ada.drive("type", "the draft is ready");
    await ada.drive("click", "#chat.send");
    await test.drive("wait", "the draft is ready");
    await test.drive("click", "#chat.draft");
    await test.drive("type", "reading it now");
    await test.drive("click", "#chat.send");
    await ada.drive("wait", "reading it now");
    await test.drive("wait", "reading it now");
    await ada.drive("wait", "the draft is ready");

    expect(refused.length === 0, "neither page made a file call", refused);

    // A file call made on the page fails at once, naming itself.
    const errno = await test.page.evaluate(async () => {
        const wasi = await import(new URL("wasi.js", document.baseURI).href);
        return wasi.path_open(3, 0, 0, 0, 0, 0n, 0n, 0, 0);
    });
    expect(errno !== 0, "a file call on the page fails", errno);
    await test.until(() => refused.length > 0, "the page saying it refused a file call");
    expect(
        refused.length === 1 && refused[0].includes("path_open"),
        "the page names the file call it refused, once",
        refused,
    );
}
