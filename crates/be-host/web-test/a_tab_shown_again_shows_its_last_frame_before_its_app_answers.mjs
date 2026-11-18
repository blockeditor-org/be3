// A view the host shows again (a text's body and format bar, after a tab switch) shows its last
// frame until its app draws it anew, as natively (guides/apps/runtime.md, Layout and present
// commits): the page keeps a hidden view's canvas, with what the app's worker last drew into it,
// and shows that canvas again, rather than a fresh one that stays empty until the app answers.
//
// Then, with nothing left to do, the page runs no frames (guides/apps/runtime.md, Late answers
// and idle editors): over an idle spell it asks the browser for no animation frames and the
// runner runs none but the few the drive commands that measure it take. A page that drew
// nothing every frame would run hundreds.

// The idle spell, and the frames the commands that measure it take (3 to 5 seen).
const IDLE_MS = 6000;
const MEASURING_FRAMES = 8;

export default async function (test) {
    const { drive, page } = test;
    for (const name of ["One", "Two"]) {
        await drive("click", "#file-tree.new");
        await drive("click", 'MenuItem "New in My files…"');
        await drive("wait", "#picker.more");
        await drive("click", 'MenuItem "Text"');
        await drive("wait", 'TextInput "Name" value="Text"');
        await drive("click", 'TextInput "Name" value="Text"');
        await drive("key", "ctrl+a");
        await drive("type", name);
        await drive("key", "Enter");
        await drive("wait", `TreeItem "${name}"`);
    }
    await drive("wait", 'Tab "Two" selected');
    await drive("click", 'Tab "One"');
    await drive("wait", 'Tab "One" selected');
    await drive("wait", "#text.surface");
    await drive("settle");
    await drive("pause", "500");

    // The screens shown while One's tab is, marked; then every screen canvas that shows from the
    // switch back to One on, noted as it shows.
    const shown = (canvas) =>
        canvas.dataset.kind === "screen" && canvas.isConnected && getComputedStyle(canvas).display !== "none";
    const marked = await page.evaluate(`(() => {
        const shown = ${shown};
        const canvases = [...document.querySelectorAll("canvas")].filter(shown);
        for (const canvas of canvases) canvas.beforeTheSwitch = true;
        return canvases.length;
    })()`);
    if (marked === 0) throw new Error("One's tab shows no app screen");

    await drive("click", 'Tab "Two"');
    await drive("wait", 'Tab "Two" selected');
    await drive("settle");
    await drive("pause", "500");

    await page.evaluate(`(() => {
        const shown = ${shown};
        window.freshScreens = [];
        const note = () => {
            for (const canvas of document.querySelectorAll("canvas")) {
                if (shown(canvas) && !canvas.beforeTheSwitch && !canvas.afterTheSwitch) {
                    canvas.afterTheSwitch = true;
                    window.freshScreens.push(canvas.getAttribute("style"));
                }
            }
        };
        for (const id of ["apps", "layers"]) {
            new MutationObserver(note).observe(document.getElementById(id), {
                childList: true,
                subtree: true,
                attributes: true,
                attributeFilter: ["style"],
            });
        }
    })()`);
    await drive("click", 'Tab "One"');
    await drive("wait", 'Tab "One" selected');
    await drive("wait", "#text.surface");
    await drive("settle");
    await drive("pause", "500");

    const fresh = await page.evaluate("window.freshScreens");
    if (fresh.length > 0) {
        throw new Error(
            `shown again, One's tab showed ${fresh.length} canvas(es) that were not its last frame, ` +
                `empty until its app drew them: ${JSON.stringify(fresh)}`,
        );
    }
    const back = await page.evaluate(`(() => {
        const shown = ${shown};
        return [...document.querySelectorAll("canvas")].filter(shown).filter((canvas) => canvas.beforeTheSwitch).length;
    })()`);
    if (back !== marked) {
        throw new Error(`One's tab showed ${marked} screen(s) before the switch and ${back} of them after it`);
    }

    // A focused caret blinks for ten seconds after it last moved, asking for frames as it should
    // (CARET_BLINKS_FOR); the spell starts after that.
    await drive("pause", "11000");
    await page.evaluate(`(() => {
        window.animationFrames = 0;
        const request = window.requestAnimationFrame.bind(window);
        window.requestAnimationFrame = (callback) =>
            request((time) => {
                window.animationFrames++;
                return callback(time);
            });
    })()`);
    await drive("frames");
    await drive("pause", String(IDLE_MS));
    const ran = await drive("frames");
    const animationFrames = await page.evaluate("window.animationFrames");
    const runnerFrames = Number(/^window: (\d+) frames/m.exec(ran)?.[1]);
    if (!(animationFrames <= MEASURING_FRAMES && runnerFrames <= MEASURING_FRAMES)) {
        throw new Error(
            `idle for ${IDLE_MS} ms, the page ran ${animationFrames} animation frame(s) and its runner ` +
                `${runnerFrames} frame(s), more than the ${MEASURING_FRAMES} measuring it takes:\n${ran}`,
        );
    }
}
