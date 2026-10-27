// Hydrates the page be-server rendered (crates/be-portal-ui/src/lib.rs,
// render()) from the page model it embedded beside it. The forms post
// natively before this runs, and keep posting natively after.
import { bindMemory } from "./wasi.js";
import { bindThreads } from "./threads.js";
import init, * as portal from "./be_portal_ui.js";

const page = document.getElementById("be-portal-page")?.textContent;
if (page && globalThis.crossOriginIsolated) {
    const module = await WebAssembly.compileStreaming(fetch(new URL("./be_portal_ui_bg.wasm", import.meta.url)));
    const wasm = await init({ module_or_path: module });
    bindMemory(wasm.memory);
    bindThreads(module, wasm.memory);
    portal.start(page);
}
