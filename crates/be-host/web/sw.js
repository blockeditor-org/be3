// The service worker that opens the host without a network.
//
// Every request of the page's own origin, apart from the server's (/api and
// /auth), goes to the network first, so an online page always gets what is
// deployed, and each answer is kept. When the network fails, the kept answer
// is served instead: the page, the module, the shims and every app the host
// loaded before. A kept answer carries its headers, so the page is still
// cross-origin isolated when it comes from here.
//
// The page registers this at the root of its address (host.js), which is
// where it has to be to answer for the page itself: the bundle's root here,
// and /sw.js beside the entry point when deployed (crates/be-tool-local deploy).

const CACHE = "be-host";

// The page itself is kept at once: the load that registered this worker
// fetched it before there was anything to keep it.
self.addEventListener("install", (event) => {
    self.skipWaiting();
    event.waitUntil(caches.open(CACHE).then((cache) => cache.add(self.registration.scope)));
});

self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

function kept(request) {
    if (request.method !== "GET" || request.headers.has("range")) {
        return false;
    }
    const url = new URL(request.url);
    if (url.origin !== self.location.origin) {
        return false;
    }
    return !(url.pathname.startsWith("/api") || url.pathname === "/auth" || url.pathname.startsWith("/auth/"));
}

// How long the network may stay silent before a kept answer is served, as on
// a connection that is up but passes nothing: the fetch goes on and keeps what
// it gets for the next load.
const PATIENCE = 3000;

async function keptAnswer(cache, request) {
    const found = await cache.match(request, { ignoreVary: true });
    if (found || request.mode !== "navigate") {
        return found;
    }
    // An address the page was opened at with other parameters still finds the
    // page.
    return cache.match(request, { ignoreSearch: true, ignoreVary: true });
}

async function answer(request) {
    const cache = await caches.open(CACHE);
    // A deployed build's own files are under /data/BUILD and never change, so
    // a kept one is served without asking. Any other file of the bundle's root
    // keeps its name across builds and is revalidated, since the browser's
    // cache would otherwise mix two builds' module and glue.
    const immutable = new URL(request.url).pathname.startsWith("/data/");
    if (immutable) {
        const found = await cache.match(request, { ignoreVary: true });
        if (found) {
            return found;
        }
    }
    const fetched = fetch(request, immutable ? {} : { cache: "no-cache" }).then(async (response) => {
        if (response.ok && response.type === "basic") {
            await cache.put(request, response.clone());
        }
        return response;
    });
    let timer;
    const silent = new Promise((resolve) => (timer = setTimeout(resolve, PATIENCE)));
    try {
        const first = await Promise.race([fetched, silent]);
        if (first instanceof Response) {
            return first;
        }
        const found = await keptAnswer(cache, request);
        if (found) {
            fetched.catch(() => {});
            return found;
        }
        return await fetched;
    } catch (error) {
        const found = await keptAnswer(cache, request);
        if (found) {
            return found;
        }
        throw error;
    } finally {
        clearTimeout(timer);
    }
}

self.addEventListener("fetch", (event) => {
    if (kept(event.request)) {
        event.respondWith(answer(event.request));
    }
});

// Push notifications (plans/notifications.md).
//
// The browser decrypts a Web Push payload (RFC 8291) before the worker sees it,
// so `event.data` is a be_block_protocol::PushMessage: 16 bytes each of
// organization, block and sender, then the sealed text, which only the page can
// open. The worker therefore asks an open window to open it, and shows what the
// window answers; with no window, no answer in time or no key, it shows a
// notification naming nothing, since a push must always show one.

// How long a window may take to answer, and how long a shown push is kept to
// recognise a repeat of: MAX_NOTIFICATION_AGE_MILLISECONDS, which a device
// refuses a push older than.
const ANSWER_WITHIN = 3000;
const KEEP_SHOWN = 2 * 24 * 60 * 60 * 1000;
const GENERIC = { title: "New message", body: "" };

function uuid(bytes, at) {
    const hex = Array.from(bytes.subarray(at, at + 16), (byte) => byte.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

// The ids a push names, or none for bytes that are not one.
function pushed(bytes) {
    if (bytes.length < 48) {
        return null;
    }
    return { organization: uuid(bytes, 0), block: uuid(bytes, 16), sender: uuid(bytes, 32) };
}

async function digest(bytes) {
    const hash = await crypto.subtle.digest("SHA-256", bytes.subarray(48));
    return Array.from(new Uint8Array(hash).subarray(0, 16), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function shown() {
    return new Promise((resolve, reject) => {
        const request = indexedDB.open("be-push", 1);
        request.onupgradeneeded = () => request.result.createObjectStore("shown");
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}

// Records `id` as shown, answering whether it already was within KEEP_SHOWN.
// The dropped are those older than that, so the store never grows.
async function seen(id) {
    const database = await shown();
    try {
        return await new Promise((resolve, reject) => {
            const transaction = database.transaction("shown", "readwrite");
            const store = transaction.objectStore("shown");
            const now = Date.now();
            let repeat = false;
            const read = store.get(id);
            read.onsuccess = () => {
                repeat = typeof read.result === "number" && now - read.result < KEEP_SHOWN;
                if (!repeat) {
                    store.put(now, id);
                }
            };
            store.openCursor().onsuccess = (event) => {
                const cursor = event.target.result;
                if (cursor) {
                    if (now - cursor.value >= KEEP_SHOWN) {
                        cursor.delete();
                    }
                    cursor.continue();
                }
            };
            transaction.oncomplete = () => resolve(repeat);
            transaction.onerror = () => reject(transaction.error);
        });
    } finally {
        database.close();
    }
}

// What the first window to answer says to `bytes`: { title, body } of the
// notification, or null. Every window is asked at once, since one that is
// waiting for another tab to close never answers.
async function asked(bytes) {
    const clients = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    const answers = clients.map(
        (client) =>
            new Promise((resolve) => {
                const channel = new MessageChannel();
                const timer = setTimeout(() => resolve(null), ANSWER_WITHIN);
                channel.port1.onmessage = (event) => {
                    clearTimeout(timer);
                    resolve(event.data && typeof event.data.title === "string" ? event.data : null);
                };
                client.postMessage({ push: bytes }, [channel.port2]);
            }),
    );
    const settled = answers.map((answer) => answer.then((value) => value ?? new Promise(() => {})));
    return Promise.race([...settled, Promise.all(answers).then(() => null)]);
}

async function show(data) {
    const bytes = new Uint8Array(await data.arrayBuffer());
    const ids = pushed(bytes);
    if (ids && (await seen(await digest(bytes)))) {
        // The server resent one already shown: a push must show something, but
        // not twice, so the one shown stays.
        return;
    }
    const answer = ids ? await asked(bytes) : null;
    const { title, body } = answer ?? GENERIC;
    await self.registration.showNotification(title, {
        body,
        // A later one about the same block replaces the earlier; one naming
        // nothing is not about any.
        tag: answer && ids ? ids.block : undefined,
        data: answer && ids ? { organization: ids.organization, block: ids.block } : {},
    });
}

self.addEventListener("push", (event) => {
    if (event.data) {
        event.waitUntil(show(event.data));
    } else {
        event.waitUntil(self.registration.showNotification(GENERIC.title));
    }
});

// A press opens the block in a window, focusing one that is open.
self.addEventListener("notificationclick", (event) => {
    event.notification.close();
    const open = event.notification.data?.block ? event.notification.data : null;
    event.waitUntil(
        (async () => {
            const clients = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
            const client = clients[0];
            if (client) {
                await client.focus();
                if (open) {
                    client.postMessage({ open });
                }
            } else {
                const query = open ? `?open-block=${open.organization}/${open.block}` : "";
                await self.clients.openWindow(new URL(query, self.registration.scope).href);
            }
        })(),
    );
});
