import { bindMemory, bindFiles, CONTROL } from "./wasi.js";
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

            function show(text) {
                status.hidden = false;
                status.innerHTML = "";
                const body = document.createElement("div");
                body.className = "detail";
                body.textContent = text;
                status.append(body);
            }

            // One tab at a time owns the files: a second waits here until the
            // first closes, and then starts. The lock is held until the page
            // goes away.
            async function ownTheFiles() {
                if (!navigator.locks) {
                    return;
                }
                const held = (resolve) => (lock) => {
                    resolve(lock !== null);
                    return lock === null ? undefined : new Promise(() => {});
                };
                const name = "block-files";
                const owned = await new Promise((resolve) =>
                    navigator.locks.request(name, { ifAvailable: true }, held(resolve)),
                );
                if (!owned) {
                    show("Block is open in another tab. It opens here once that tab is closed.");
                    await new Promise((resolve) => navigator.locks.request(name, held(resolve)));
                }
            }

            // The worker that answers the module's file calls (files.js), and
            // the shared channel they reach it through.
            async function startFiles() {
                const channel = {
                    control: new SharedArrayBuffer(CONTROL.WORDS * 4),
                    args: new SharedArrayBuffer(CONTROL.ARGUMENTS * 8),
                };
                const worker = new Worker(new URL("./files.js", import.meta.url), { type: "module" });
                const ready = new Promise((resolve, reject) => {
                    worker.onmessage = (event) => resolve(event.data);
                    worker.onerror = (event) => reject(new Error(`the file worker failed: ${event.message}`));
                });
                worker.postMessage(channel);
                await ready;
                return { worker, channel };
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
                await ownTheFiles();
                navigator.storage?.persist?.().catch(() => {});
                const files = await startFiles();
                const module = await WebAssembly.compileStreaming(fetch("./block_app_lib_bg.wasm"));
                const wasm = await init({ module_or_path: module });
                // The glue runs the module's start function before returning,
                // so this is the earliest the shims can be given memory.
                bindMemory(wasm.memory);
                bindEnvironmentMemory(wasm.memory);
                bindThreads(module, wasm.memory);
                files.worker.postMessage({ memory: wasm.memory });
                bindFiles(files.channel);

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
