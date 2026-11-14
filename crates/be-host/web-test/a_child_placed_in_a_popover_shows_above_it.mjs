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
    // In a browser the field's focus is the page's, which a click gives it.
    await drive("wait", 'TextInput "Name" value="Canvas"');
    await drive("click", 'TextInput "Name" value="Canvas"');
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

    // The child's screen stacks in #layers above the popover's overlay, as the host places them
    // from its last frame: waited for, with what the page held last on failure.
    const stacking = () => {
        const shown = (kind) =>
            [...document.querySelectorAll(`#layers canvas[data-kind="${kind}"]`)]
                .filter((canvas) => getComputedStyle(canvas).visibility !== "hidden")
                .map((canvas) => Number(getComputedStyle(canvas).zIndex) || 0);
        const screens = shown("screen");
        const overlays = shown("overlay");
        const above = screens.length > 0 && overlays.length > 0 && Math.max(...screens) > Math.max(...overlays);
        return { above, screens, overlays, apps: document.querySelectorAll('#apps canvas[data-kind="screen"]').length };
    };
    try {
        await test.page.waitForFunction(`(${stacking})().above`, null, { timeout: 30000 });
    } catch {
        const last = await test.page.evaluate(`(${stacking})()`);
        throw new Error(`the child's screen did not stack above the popover: ${JSON.stringify(last)}`);
    }
    // The Debug view's own screen, painted before the popover, stays under the host's canvas.
    const held = await test.page.evaluate(`(${stacking})()`);
    if (held.apps === 0) {
        throw new Error(`no screen stayed in #apps under the host's canvas: ${JSON.stringify(held)}`);
    }
}
