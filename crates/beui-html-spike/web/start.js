// Hydrates the page :ssr prerendered (src/lib.rs, page()). Before the module
// loads it notes every prerendered element and starts collecting console
// warnings, so that checks() can say whether hydration kept the elements it
// was given and found nothing to rebuild. buck/app/web-test.mjs waits on
// beuiStarted and calls automate(words) and checks() on what it resolves to.
import { bindMemory } from "./wasi.js";
import { bindThreads } from "./threads.js";
import init, * as spike from "./beui_html_spike.js";

const app = document.getElementById("app");
const prerendered = [...app.querySelectorAll("*")];
const prerenderedText = app.textContent;
const warnings = [];
const warn = console.warn.bind(console);
console.warn = (...parts) => {
    warnings.push(parts.join(" "));
    warn(...parts);
};

function checks() {
    const lines = [];
    const check = (passed, name, why) => lines.push(passed ? `ok ${name}` : `FAILED ${name}: ${why}`);
    check(
        prerenderedText.includes("Choose an account to continue."),
        "the page arrives prerendered",
        `#app held ${JSON.stringify(prerenderedText.slice(0, 80))} before the module loaded`,
    );
    const replaced = prerendered.filter((element) => !element.isConnected);
    check(
        prerendered.length > 0 && replaced.length === 0,
        "hydration keeps every prerendered element",
        `${replaced.length} of ${prerendered.length} were replaced, the first <${replaced[0]?.localName}>`,
    );
    const mismatches = warnings.filter((warning) => warning.includes("hydration mismatch"));
    check(mismatches.length === 0, "hydration finds nothing to rebuild", mismatches.join("; "));
    return lines.join("\n") + "\n";
}

globalThis.beuiStarted = (async () => {
    const module = await WebAssembly.compileStreaming(fetch("./beui_html_spike_bg.wasm"));
    const wasm = await init({ module_or_path: module });
    bindMemory(wasm.memory);
    bindThreads(module, wasm.memory);
    spike.start();
    return { automate: spike.automate, checks };
})();
