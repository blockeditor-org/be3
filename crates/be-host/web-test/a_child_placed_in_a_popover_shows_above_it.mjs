// A child an app places above its own popover shows over it on the web too: the Debug view of
// a canvas previews the canvas's first block, a text, in the text's own editor. Natively this is
// crates/be-host-tests' test of the same name; here the child's canvas must stack above the
// popover's overlay canvas in #layers, not behind the host's canvas in #apps.
export default async function (test) {
    const { drive } = test;
    await drive("click", "#file-tree.new");
    await drive("click", 'MenuItem "New in My files…"');
    await drive("wait", "#picker.more");
    await drive("click", 'MenuItem "Canvas"');
    await drive("wait", 'Pane "Canvas"');
    await drive("wait", 'TextInput "Name" value="Canvas" focused');
    await drive("key", "Enter");
    await drive("wait", "not", 'TextInput "Name" value="Canvas"');
    await drive("wait", 'TreeItem "Canvas"');
    await drive("click", "#infinite-canvas.dock.block");
    await drive("wait", "#picker.more");
    await drive("click", 'MenuItem "Text"');
    await drive("wait", "GenericContainer #infinite-canvas.entity.");
    const tree = await drive("tree");
    const close = tree
        .split("\n")
        .slice(tree.split("\n").findIndex((line) => line.includes('Tab "Canvas"')))
        .flatMap((line) => line.trim().split(/\s+/))
        .find((word) => word.startsWith("#dock.tab.") && word.endsWith(".close"));
    await drive("click", close);
    await drive("wait", "not", 'Tab "Canvas"');
    await drive("click", 'TreeItem "Canvas"', "right");
    await drive("click", "#file-tree.open-with");
    await drive("click", "#file-tree.open-with.777b1dfa-5369-402e-b22d-7eb56877ad4d");
    await drive("click", "#block-debug.preview");
    await drive("wait", "#block-debug.preview.child");
    await drive("wait", 'Pane "Text"');
    await drive("pause", "500");

    const stacked = await test.page.evaluate(() =>
        [...document.querySelectorAll("#layers canvas")]
            .filter((canvas) => getComputedStyle(canvas).visibility !== "hidden")
            .map((canvas) => ({
                screen: canvas.style.transform !== "",
                z: Number(getComputedStyle(canvas).zIndex) || 0,
            })),
    );
    const screens = stacked.filter((canvas) => canvas.screen);
    const overlays = stacked.filter((canvas) => !canvas.screen);
    if (screens.length === 0) {
        throw new Error(`the child's screen is not in #layers: ${JSON.stringify(stacked)}`);
    }
    if (overlays.length === 0) {
        throw new Error(`the popover has no overlay in #layers: ${JSON.stringify(stacked)}`);
    }
    const top = Math.max(...overlays.map((canvas) => canvas.z));
    if (!screens.some((canvas) => canvas.z > top)) {
        throw new Error(`the child's screen is not above the popover: ${JSON.stringify(stacked)}`);
    }
}
