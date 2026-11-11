import { bindMemory, bindFiles, CONTROL } from "./wasi.js";
            import { bindMemory as bindEnvironmentMemory } from "./env.js";
            import { bindThreads } from "./threads.js";
            import init, {
                run_web,
                push_open,
                push_open_block,
                push_register,
                push_server_key,
                push_unregister,
            } from "./be_host.js";
            import {
                bindPush,
                currentPush,
                pushPermission,
                pushState,
                subscribePush,
                unsubscribePush,
            } from "./push.js";

            const status = document.getElementById("status");

            // Resolved once the module can be called, so drive waits for it
            // after a reload; automation asks made before the app runs wait
            // in its inbox.
            let started;
            globalThis.blockStarted = new Promise((resolve, reject) => {
                started = { resolve, reject };
            });
            globalThis.blockStarted.catch(() => {});

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
                const name = "be-host-files";
                const owned = await new Promise((resolve) =>
                    navigator.locks.request(name, { ifAvailable: true }, held(resolve)),
                );
                if (!owned) {
                    show("be-host is open in another tab. It opens here once that tab is closed.");
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
                // Kept for opening offline (sw.js), registered beside the
                // page's own address rather than the bundle's base, since a
                // service worker only answers for addresses below its own.
                navigator.serviceWorker
                    ?.register(new URL("sw.js", location.href))
                    .catch((error) => console.warn("the app cannot be kept for offline use", error));
                await ownTheFiles();
                navigator.storage?.persist?.().catch(() => {});
                const files = await startFiles();
                const module = await WebAssembly.compileStreaming(fetch("./be_host_bg.wasm"));
                const wasm = await init({ module_or_path: module });
                // The glue runs the module's start function before returning,
                // so this is the earliest the shims can be given memory.
                bindMemory(wasm.memory);
                bindEnvironmentMemory(wasm.memory);
                bindThreads(module, wasm.memory);
                files.worker.postMessage({ memory: wasm.memory });
                bindFiles(files.channel);
                bindPush({
                    open: push_open,
                    openBlock: ({ organization, block }) => push_open_block(organization, block),
                    changed: () => globalThis.bePush.sync().catch(() => {}),
                });
                // What the host's "Notifications" switch calls (plans/notifications.md):
                // `enable` must run in a person's gesture.
                globalThis.bePush = {
                    permission: pushPermission,
                    state: pushState,
                    // The address of this browser's subscription, or null.
                    async address() {
                        return (await currentPush())?.address ?? null;
                    },
                    unsubscribe: unsubscribePush,
                    async enable() {
                        const { address, p256dh, auth, replaced } = await subscribePush(await push_server_key());
                        try {
                            if (replaced) {
                                await push_unregister(replaced).catch(() => {});
                            }
                            await push_register(address, p256dh, auth);
                        } catch (error) {
                            // Not registered, so not on: leave no subscription behind.
                            await unsubscribePush().catch(() => {});
                            throw error;
                        }
                        return address;
                    },
                    async disable() {
                        const address = await unsubscribePush();
                        if (address) {
                            await push_unregister(address);
                        }
                    },
                    // Tells the server about this browser's subscription for the open
                    // organization: it is registered per organization, and the push service
                    // may have replaced it while no page was open.
                    async sync() {
                        const current = await currentPush();
                        if (current) {
                            await push_register(current.address, current.p256dh, current.auth);
                        }
                    },
                };
                // A page opened by pressing a notification names its block in the address.
                try {
                    const url = new URL(location.href);
                    const opened = url.searchParams.get("open-block");
                    if (opened !== null) {
                        const [organization, block, ...rest] = opened.split("/");
                        const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
                        if (rest.length === 0 && uuid.test(organization) && uuid.test(block)) {
                            push_open_block(organization, block);
                        }
                        url.searchParams.delete("open-block");
                        history.replaceState(history.state, "", url);
                    }
                } catch (error) {
                    console.warn("the block a notification named could not be opened", error);
                }
                started.resolve();

                status.hidden = true;
                await run_web("host");
            } catch (error) {
                console.error(error);
                started.reject(error);
                fail(
                    "be-host could not start.",
                    `${error}\n\nbe-host draws with WebGPU or WebGL through a canvas. ` +
                        "Check the browser console for details.",
                );
            }
