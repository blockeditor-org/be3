import { bindMemory } from "./wasi.js";
            import { bindThreads } from "./threads.js";
            import init, { run_demo } from "./beui_web_demo.js";

            const status = document.getElementById("status");
            try {
                if (!self.crossOriginIsolated) {
                    throw new Error(
                        "the page is not cross-origin isolated, so it cannot hold the module's " +
                            "shared memory; serve it with Cross-Origin-Opener-Policy: same-origin " +
                            "and Cross-Origin-Embedder-Policy: require-corp",
                    );
                }
                const module = await WebAssembly.compileStreaming(fetch("./beui_web_demo_bg.wasm"));
                const wasm = await init({ module_or_path: module });
                bindMemory(wasm.memory);
                bindThreads(module, wasm.memory);
                status.hidden = true;
                await run_demo("demo");
            } catch (error) {
                console.error(error);
                status.hidden = false;
                status.textContent = `The demo could not start.\n\n${error}`;
            }
