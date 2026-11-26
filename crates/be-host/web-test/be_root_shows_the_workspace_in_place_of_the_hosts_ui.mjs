// be-root in place of be-host's own UI on the web (`?root=be3.root`), as `--root` natively: the
// root runs on the page and shows the signed-in workspace, its tree reaches drive as be-host's
// own UI's does, and the page shows its frame in canvases of its own in #layers over the host's
// canvas: the root's own surface, which it draws on the page's device, and the screens of the
// apps it embeds (the workspace's files among them), which their workers draw.
export default async function (test) {
    // One tab at a time holds the host's files (the `block-files` Web Lock).
    await test.page.close();
    // Which waits for the workspace's files in the root's tree, as for be-host's own UI.
    await test.openPage("root=be3.root");
    const { page } = test;

    // The colour at the middle of each canvas the page shows the root's frame in, by its kind,
    // once each kind shows something there.
    const shown = await test.until(
        () =>
            page.evaluate(() => {
                const shows = {};
                for (const canvas of document.querySelectorAll("#layers canvas")) {
                    const { width, height } = canvas;
                    if (width === 0 || height === 0) continue;
                    const reader = new OffscreenCanvas(width, height).getContext("2d");
                    reader.drawImage(canvas, 0, 0);
                    const [, , , alpha] = reader.getImageData(width >> 1, height >> 1, 1, 1).data;
                    if (alpha !== 0) shows[canvas.dataset.kind] = true;
                }
                return shows.root && shows.screen ? shows : undefined;
            }),
        "the root's canvas and an app's screen to show something",
    );
    if (!shown.root || !shown.screen) {
        throw new Error(`the page shows the root's frame in ${JSON.stringify(shown)}`);
    }
}
