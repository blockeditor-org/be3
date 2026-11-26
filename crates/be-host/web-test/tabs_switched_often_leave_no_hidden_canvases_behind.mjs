// Under a root on the page (`test.root`), a canvas whose layer leaves the root's list stays
// hidden only while its app still holds the surface (`Root::holds_app_surface`), and leaves the
// page once the app's worker reports letting it go (app.js's `released`). A text shown again
// comes back on new surfaces, so switching between two texts' tabs again and again would leave
// a hidden canvas for every surface each showing had, were the releases not heard; heard, the
// page holds no more screen canvases than it shows, and a few hidden ones besides.

const SWITCHES = 6;
// The hidden screen canvases a page may hold at once: the views of the tab hidden last, a text's
// body and format bar, and the two its app has yet to let go of.
const HIDDEN_AT_MOST = 4;

export default async function (test) {
    const { drive, page } = test;
    // A click settles before what the root's apps show for it does (be-host's `busy` does not
    // yet say whether they settled), so each press waits for its target first.
    const press = async (target, ...rest) => {
        await drive("wait", target);
        await drive("click", target, ...rest);
    };
    // Pressed as soon as the picker shows it, its Text now and then makes no creation under a
    // root (plans/host_as_platform_adapter.md, 4g7g2c4b): pressed again while it still shows,
    // until the name field shows.
    // The page's drive takes no timeout of its own, so the tree is read for a few seconds.
    const pick = async (item, field) => {
        for (let tries = 1; tries <= 3; tries++) {
            await press(item);
            for (let read = 0; read < 20; read++) {
                const tree = await drive("--no-settle", "tree");
                if (tree.includes(field)) return;
                if (!tree.includes(item)) break;
                await drive("pause", "500");
            }
        }
        await drive("wait", field);
    };
    for (const name of ["One", "Two"]) {
        await press("#file-tree.new");
        await press('MenuItem "New in My files…"');
        await drive("wait", "#picker.more");
        await pick('MenuItem "Text"', 'TextInput "Name" value="Text"');
        await press('TextInput "Name" value="Text"');
        await drive("key", "ctrl+a");
        await drive("type", name);
        await drive("key", "Enter");
        await drive("wait", `TreeItem "${name}"`);
    }
    for (let switched = 0; switched < SWITCHES; switched++) {
        const name = switched % 2 === 0 ? "One" : "Two";
        await press(`Tab "${name}"`);
        await drive("wait", `Tab "${name}" selected`);
        await drive("wait", "#text.surface");
        await drive("settle");
        await drive("pause", "500");
    }
    const held = await page.evaluate(() => {
        const screens = [...document.querySelectorAll("#layers canvas[data-kind=screen]")];
        const shown = screens.filter((canvas) => getComputedStyle(canvas).display !== "none").length;
        return { shown, hidden: screens.length - shown };
    });
    if (held.shown === 0) throw new Error("the page shows no app screen");
    if (held.hidden > HIDDEN_AT_MOST) {
        throw new Error(
            `after ${SWITCHES} tab switches the page holds ${held.hidden} hidden screen canvas(es) beside the ` +
                `${held.shown} it shows, more than the ${HIDDEN_AT_MOST} its apps may still hold`,
        );
    }
}
