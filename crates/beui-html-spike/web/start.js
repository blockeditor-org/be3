// Hydrates the page :ssr prerendered (src/lib.rs, page()). Before the module
// loads it notes every prerendered element and starts collecting console
// warnings, so that checks() can say whether hydration kept the elements it
// was given and found nothing to rebuild. buck/app/web-test.mjs waits on
// beuiStarted and calls automate(words) and checks() on what it resolves to,
// once for each colour scheme: the prerendered page must already show the
// scheme's colours, so hydration changes none.
import { bindMemory } from "./wasi.js";
import { bindThreads } from "./threads.js";
import init, * as spike from "./beui_html_spike.js";

const app = document.getElementById("app");
const prerendered = [...app.querySelectorAll("*")];
const prerenderedText = app.textContent;
const COLORS = ["color", "background-color", "outline-color", "border-top-color"];
const colorsOf = (element) => {
    const style = getComputedStyle(element);
    return COLORS.map((property) => style.getPropertyValue(property)).join(" ");
};
const prerenderedColors = prerendered.map(colorsOf);
const COLOR = /#[0-9a-f]{6,8}|rgba?\([^)]*\)|light-dark\((?:[^()]|\([^)]*\))*\)|transparent/g;
const colorsIn = (style) => (style ?? "").match(COLOR)?.join(" ") ?? "";
const recolored = [];
new MutationObserver((records) => {
    for (const record of records) {
        const now = record.target.getAttribute("style");
        if (colorsIn(record.oldValue) !== colorsIn(now)) {
            recolored.push(`<${record.target.localName}> ${record.oldValue} -> ${now}`);
        }
    }
}).observe(app, { subtree: true, attributes: true, attributeFilter: ["style"], attributeOldValue: true });
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
    check(
        recolored.length === 0,
        "hydration rewrites no colour",
        `${recolored.length} styles changed colour, the first ${recolored[0]}`,
    );
    const changed = prerendered.filter((element, index) => colorsOf(element) !== prerenderedColors[index]);
    check(
        changed.length === 0,
        `the prerendered page has the hydrated page's colours (${matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"})`,
        `${changed.length} elements changed colour, the first <${changed[0]?.localName}> from ${prerenderedColors[prerendered.indexOf(changed[0])]} to ${changed[0] && colorsOf(changed[0])}`,
    );
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
