// The first frame after the page grows shows the workspace's backdrop beyond its last frame
// (guides/apps/runtime.md, Late answers). The host draws its own canvas at the new size in that
// frame, but the workspace's canvas, drawn by its app's worker, keeps its old frame at its old
// size until the app's frame of the new size arrives, always a later page frame since the page
// cannot wait on a worker. Beyond that old frame the host's canvas shows the view's backdrop
// (`Backdrop` in be-host's app_view.rs, filling the view's laid box), not the host's own clear
// colour and not a hole onto the page behind it.
//
// The workspace clears to the theme's background, as the host does, so the test tells the two
// apart by having the page's WebGL clear to magenta wherever the host's renderer clears to an
// opaque colour: without the backdrop, the gap would read magenta. The app canvases draw in
// their workers, which the init script does not reach, so they are unchanged. This assumes the
// host's only opaque clear is the one that starts its frame; a renderer that cleared to an opaque
// colour anywhere else (a backdrop drawn as a clear, say) would read magenta there too.

const GROWN = { width: 1160, height: 700 };
const CLEARED = [255, 0, 255];

// Runs in the page before its scripts: keeps the host canvas's drawing buffer readable after the
// frame that drew it, and has every opaque colour clear clear to magenta instead.
function readableAndMagenta() {
    const getContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (kind, options) {
        if (kind === "webgl2" || kind === "webgl") options = { ...options, preserveDrawingBuffer: true };
        return getContext.call(this, kind, options);
    };
    window.opaqueClears = 0;
    for (const Context of [window.WebGL2RenderingContext, window.WebGLRenderingContext]) {
        if (!Context) continue;
        const clearColor = Context.prototype.clearColor;
        Context.prototype.clearColor = function (red, green, blue, alpha) {
            if (alpha > 0) {
                window.opaqueClears++;
                return clearColor.call(this, 1, 0, 1, 1);
            }
            return clearColor.call(this, red, green, blue, alpha);
        };
        const clearBufferfv = Context.prototype.clearBufferfv;
        if (clearBufferfv) {
            Context.prototype.clearBufferfv = function (buffer, drawBuffer, values, ...rest) {
                if (buffer === this.COLOR && values[3] > 0) {
                    window.opaqueClears++;
                    return clearBufferfv.call(this, buffer, drawBuffer, new Float32Array([1, 0, 1, 1]));
                }
                return clearBufferfv.call(this, buffer, drawBuffer, values, ...rest);
            };
        }
    }
}

export default async function (test) {
    await test.context.addInitScript(readableAndMagenta);
    await test.page.close();
    await test.openPage();
    const { drive, page } = test;
    await drive("settle");
    // Settled, the workspace has drawn its screen: wait for its canvas to show, rather than for
    // a length of time.
    await page.waitForFunction(() =>
        [...document.querySelectorAll("#apps canvas[data-kind=screen]")].some(
            (canvas) => getComputedStyle(canvas).display !== "none" && canvas.getBoundingClientRect().width > 0,
        ),
    );

    // The workspace's canvas: the largest app screen shown, which fills the page.
    const before = await page.evaluate(() => {
        const screens = [...document.querySelectorAll("#apps canvas[data-kind=screen]")]
            .filter((canvas) => getComputedStyle(canvas).display !== "none")
            .map((canvas) => ({ canvas, rect: canvas.getBoundingClientRect() }))
            .sort((a, b) => b.rect.width * b.rect.height - a.rect.width * a.rect.height);
        if (screens.length === 0) return null;
        screens[0].canvas.isTheWorkspace = true;
        const { left, top, right, bottom } = screens[0].rect;
        return { left, top, right, bottom, opaqueClears: window.opaqueClears };
    });
    if (before === null) throw new Error("the page shows no app screen");
    if (before.opaqueClears === 0) {
        throw new Error("the host's renderer made no opaque WebGL clear, so the test cannot tell its clear colour");
    }
    if (before.right > GROWN.width - 100 || before.bottom > GROWN.height - 50) {
        throw new Error(`the workspace's canvas, at ${JSON.stringify(before)}, leaves no room to grow into`);
    }

    // In the ResizeObserver report of the grown size, the host's canvas holds the first frame of
    // that size (the resize's animation frame ran before it): read it there, with where the
    // workspace's canvas is then. A new observer reports its starting size first, so the report
    // is picked by size.
    const seen = page.evaluate(
        ({ grown, before }) =>
            new Promise((resolve) => {
                const host = document.getElementById("host");
                const observer = new ResizeObserver((entries) => {
                    const entry = entries.find(
                        (entry) =>
                            Math.round(entry.contentRect.width) === grown.width &&
                            Math.round(entry.contentRect.height) === grown.height,
                    );
                    if (!entry) return;
                    observer.disconnect();
                    const workspace = [...document.querySelectorAll("#apps canvas")].find(
                        (canvas) => canvas.isTheWorkspace,
                    );
                    const shown = workspace?.getBoundingClientRect();
                    const scale = host.width / host.clientWidth;
                    const copy = document.createElement("canvas");
                    copy.width = host.width;
                    copy.height = host.height;
                    const context = copy.getContext("2d");
                    context.drawImage(host, 0, 0);
                    const pixel = (x, y) => [...context.getImageData(Math.round(x * scale), Math.round(y * scale), 1, 1).data];
                    const right = { x: before.right + 100, y: (before.top + before.bottom) / 2 };
                    const below = { x: (before.left + before.right) / 2, y: before.bottom + 50 };
                    resolve({
                        host: [host.width, host.height],
                        workspace: shown && { right: shown.right, bottom: shown.bottom },
                        right: { ...right, pixel: pixel(right.x, right.y) },
                        below: { ...below, pixel: pixel(below.x, below.y) },
                        background: document.body.style.background,
                    });
                });
                observer.observe(host);
            }),
        { grown: GROWN, before },
    );
    // The observer is listening once its starting report has come, which the page answers after.
    await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => resolve())));
    await page.setViewportSize(GROWN);
    const first = await seen;
    console.log(`the first frame after the page grew: ${JSON.stringify(first)}`);

    const scale = first.host[0] / GROWN.width;
    if (first.host[0] !== Math.round(GROWN.width * scale) || first.host[1] !== Math.round(GROWN.height * scale)) {
        throw new Error(`the host's canvas was not drawn at the grown size in the first frame: ${JSON.stringify(first)}`);
    }
    if (!first.workspace || first.workspace.right > before.right || first.workspace.bottom > before.bottom) {
        throw new Error(
            `the workspace's canvas was not at its old size in the first frame, so there is no gap to check: ${JSON.stringify(first)}`,
        );
    }
    // The workspace's backdrop: the theme's background, the colour the host tells the page.
    const backdrop = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(first.background)?.slice(1, 4).map(Number);
    if (!backdrop) throw new Error(`the page's background is not a colour: ${first.background}`);
    for (const [where, { pixel }] of Object.entries({ right: first.right, below: first.below })) {
        const what =
            pixel[3] < 255
                ? "a hole in the host's canvas"
                : pixel.slice(0, 3).every((value, index) => Math.abs(value - CLEARED[index]) <= 2)
                  ? "the host's clear colour"
                  : null;
        const matches = pixel.slice(0, 3).every((value, index) => Math.abs(value - backdrop[index]) <= 2);
        if (what !== null || !matches) {
            throw new Error(
                `${where} of the workspace's old frame, the first frame after the page grew showed ` +
                    `${what ?? `rgba(${pixel})`} rather than the workspace's backdrop, rgb(${backdrop}): ${JSON.stringify(first)}`,
            );
        }
    }
}
