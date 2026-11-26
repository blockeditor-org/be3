import { bindMemory } from "./wasi.js";
            import { bindThreads } from "./threads.js";
            import init, * as program from "./__MODULE__.js";

            // beui_host_window::main! exports beui_start, which shows the program in #app
            // and resolves once it runs, and automate, which drive's browser tests call
            // once beuiStarted gives them the module.
            const status = document.getElementById("status");
            globalThis.beuiStarted = (async () => {
                if (!self.crossOriginIsolated) {
                    throw new Error(
                        "the page is not cross-origin isolated, so it cannot hold the module's " +
                            "shared memory; serve it with Cross-Origin-Opener-Policy: same-origin " +
                            "and Cross-Origin-Embedder-Policy: require-corp",
                    );
                }
                const module = await WebAssembly.compileStreaming(fetch("./__MODULE___bg.wasm"));
                const wasm = await init({ module_or_path: module });
                bindMemory(wasm.memory);
                bindThreads(module, wasm.memory);
                await program.beui_start("app");
                status.hidden = true;
                return program;
            })();
            globalThis.beuiStarted.catch((error) => {
                console.error(error);
                status.hidden = false;
                status.textContent = `The program could not start.\n\n${error}`;
            });
