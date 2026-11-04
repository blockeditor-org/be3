import { bindMemory } from "./wasi.js";
            import { bindThreads } from "./threads.js";
            import init, { run_review } from "./paint_review_web.js";

            const status = document.getElementById("status");
            try {
                if (!self.crossOriginIsolated) {
                    throw new Error(
                        "the page is not cross-origin isolated, so it cannot hold the module's " +
                            "shared memory; serve it with Cross-Origin-Opener-Policy: same-origin " +
                            "and Cross-Origin-Embedder-Policy: require-corp",
                    );
                }
                const module = await WebAssembly.compileStreaming(fetch("./paint_review_web_bg.wasm"));
                const wasm = await init({ module_or_path: module });
                bindMemory(wasm.memory);
                bindThreads(module, wasm.memory);
                status.hidden = true;
                await run_review("review");
            } catch (error) {
                console.error(error);
                status.hidden = false;
                status.textContent = `The paintings could not be shown.\n\n${error}`;
            }
