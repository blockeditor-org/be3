// Instantiating an app's worker in the browser (be_sdk::worker,
// guides/adding_an_app.md, Workers): the extension be-wasm-guest-host's worker
// (src/guest.js) loads for one, which be-wasm-sandbox-host's web half starts in
// a browser worker of its own. It is linked as app.js links an app, but has no
// display: no gpu device is opened for it, and it paints nothing, so a step
// reports only the messages it sent.

import { link } from "./app.js";

export async function boot(options) {
    await link(options, false);
    return {
        receive() {},
        report() {
            return {};
        },
    };
}
