import { bindMemory } from "./wasi.js";
            import { bindMemory as bindEnvironmentMemory } from "./env.js";
            import { bindThreads } from "./threads.js";
            import init, { run_web } from "./block_app_lib.js";

            const status = document.getElementById("status");

            function fail(heading, detail) {
                status.hidden = false;
                status.innerHTML = "";
                const title = document.createElement("div");
                title.textContent = heading;
                const body = document.createElement("div");
                body.className = "detail";
                body.textContent = detail;
                status.append(title, body);
            }

            try {
                if (!self.crossOriginIsolated) {
                    throw new Error(
                        "the page is not cross-origin isolated, so it cannot hold the " +
                            "shared memory the module's threads run on; the server must send " +
                            "Cross-Origin-Opener-Policy: same-origin and " +
                            "Cross-Origin-Embedder-Policy: require-corp",
                    );
                }

                // The module is compiled here rather than inside the glue,
                // because starting a thread means instantiating this very
                // module again on a Worker, and only the page holds it.
                const module = await WebAssembly.compileStreaming(fetch("./block_app_lib_bg.wasm"));
                const wasm = await init({ module_or_path: module });
                // The glue runs the module's start function before returning,
                // so this is the earliest the shims can be given memory.
                bindMemory(wasm.memory);
                bindEnvironmentMemory(wasm.memory);
                bindThreads(module, wasm.memory);

                status.hidden = true;
                await run_web("block");
            } catch (error) {
                console.error(error);
                fail(
                    "Block could not start.",
                    `${error}\n\nBlock draws with WebGPU or WebGL through a canvas. ` +
                        "Check the browser console for details.",
                );
            }
