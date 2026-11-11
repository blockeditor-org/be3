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
// open. The worker therefore asks the open windows to open it, and shows what
// the first to answer says; with no window, no answer in time or no key, it
// shows a notification naming nothing, since a push must always show one.
// Notification text is plaintext on a lock screen: hiding it is a follow-up.

// How long a window may take to answer, and how long a shown push is kept to
// recognise a repeat of: MAX_NOTIFICATION_AGE_MILLISECONDS, which a device
// refuses a push older than.
const ANSWER_WITHIN = 3000;
const KEEP_SHOWN = 2 * 24 * 60 * 60 * 1000;
// How long a pressed notification's block waits for a window that is still loading.
const OPEN_WITHIN = 30000;
const GENERIC = { title: "New message", body: "", tag: "new-message" };

function uuid(bytes, at) {
    const hex = Array.from(bytes.subarray(at, at + 16), (byte) => byte.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

// Whether the bytes are a push: 48 bytes of ids, then the sealed text.
function isPush(bytes) {
    return bytes.length > 48;
}

async function digest(bytes) {
    const hash = await crypto.subtle.digest("SHA-256", bytes.subarray(48));
    return Array.from(new Uint8Array(hash).subarray(0, 16), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function database() {
    return new Promise((resolve, reject) => {
        const request = indexedDB.open("be-push", 1);
        request.onupgradeneeded = () => request.result.createObjectStore("shown");
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}

// What was shown for push `id` within KEEP_SHOWN ({ at, tag, data }), or null. Only the block
// it was about is kept, never what it said: the text of a sealed message must not rest on this
// device outside the keys' storage. A store that cannot be read answers null: the push shows,
// and may show twice.
async function shownBefore(id) {
    try {
        const open = await database();
        try {
            return await new Promise((resolve, reject) => {
                const read = open.transaction("shown").objectStore("shown").get(id);
                read.onsuccess = () => {
                    const found = read.result;
                    resolve(found && Date.now() - found.at < KEEP_SHOWN ? found : null);
                };
                read.onerror = () => reject(read.error);
            });
        } finally {
            open.close();
        }
    } catch {
        return null;
    }
}

// Keeps that push `id` was shown (about `tag`, the block, or none), and drops what is older
// than KEEP_SHOWN, so the store never grows. A failure is the same as not keeping it.
async function recordShown(id, tag, data) {
    try {
        const open = await database();
        try {
            await new Promise((resolve, reject) => {
                const transaction = open.transaction("shown", "readwrite");
                const store = transaction.objectStore("shown");
                const now = Date.now();
                store.put({ at: now, tag, data }, id);
                store.openCursor().onsuccess = (event) => {
                    const cursor = event.target.result;
                    if (cursor) {
                        if (now - cursor.value.at >= KEEP_SHOWN) {
                            cursor.delete();
                        }
                        cursor.continue();
                    }
                };
                transaction.oncomplete = resolve;
                transaction.onerror = () => reject(transaction.error);
            });
        } finally {
            open.close();
        }
    } catch {}
}

// Windows that can be asked, the one in use first.
async function windows() {
    const found = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    const rank = (client) => (client.focused ? 2 : 0) + (client.visibilityState === "visible" ? 1 : 0);
    return found.sort((a, b) => rank(b) - rank(a));
}

// What the first window to answer says to `bytes`: { title, body, block, organization }
// of the notification, or null. Every window is asked at once, since one that is waiting
// for another tab to close never answers.
async function asked(bytes) {
    const answers = (await windows()).map(
        (client) =>
            new Promise((resolve) => {
                const channel = new MessageChannel();
                const timer = setTimeout(() => resolve(null), ANSWER_WITHIN);
                channel.port1.onmessage = (event) => {
                    clearTimeout(timer);
                    const answer = event.data;
                    resolve(
                        answer && typeof answer.title === "string" && typeof answer.block === "string" ? answer : null,
                    );
                };
                client.postMessage({ push: bytes }, [channel.port2]);
            }),
    );
    const settled = answers.map((answer) => answer.then((value) => value ?? new Promise(() => {})));
    return Promise.race([...settled, Promise.all(answers).then(() => null)]);
}

async function show(data) {
    const bytes = new Uint8Array(await data.arrayBuffer());
    const id = isPush(bytes) ? await digest(bytes) : null;
    const before = id ? await shownBefore(id) : null;
    if (before) {
        // The server resent one already shown. What is on screen stays; a push must show
        // something, so one that was dismissed or pressed comes back naming nothing, under
        // its tag, without a sound.
        const tag = before.tag ?? GENERIC.tag;
        if ((await self.registration.getNotifications({ tag })).length === 0) {
            await self.registration.showNotification(GENERIC.title, {
                body: GENERIC.body,
                tag,
                renotify: false,
                data: before.data ?? {},
            });
        }
        return;
    }
    const answer = id ? await asked(bytes) : null;
    const notification = answer
        ? {
              title: answer.title,
              options: {
                  body: typeof answer.body === "string" ? answer.body : "",
                  // A later one about the same block replaces the earlier, and sounds.
                  tag: answer.block,
                  renotify: true,
                  data: { organization: answer.organization, block: answer.block },
              },
          }
        : { title: GENERIC.title, options: { body: GENERIC.body, tag: GENERIC.tag, data: {} } };
    await self.registration.showNotification(notification.title, notification.options);
    if (id) {
        await recordShown(id, notification.options.tag, notification.options.data);
    }
}

self.addEventListener("push", (event) => {
    event.waitUntil(
        (async () => {
            try {
                if (event.data) {
                    return await show(event.data);
                }
            } catch (error) {
                console.error("a push could not be handled", error);
            }
            await self.registration.showNotification(GENERIC.title, { tag: GENERIC.tag });
        })(),
    );
});

// The block a press named, held until a window that was still loading says it is ready.
let pending = null;

// A press opens the block in a window, focusing one that is open.
self.addEventListener("notificationclick", (event) => {
    event.notification.close();
    const named = event.notification.data?.block ? event.notification.data : null;
    event.waitUntil(
        (async () => {
            const [client] = await windows();
            if (client) {
                try {
                    await client.focus();
                } catch {}
                if (named) {
                    pending = { open: named, at: Date.now() };
                    client.postMessage({ open: named });
                }
            } else {
                const query = named ? `?open-block=${named.organization}/${named.block}` : "";
                await self.clients.openWindow(new URL(query, self.registration.scope).href);
            }
        })(),
    );
});

// A page that has bound its listener says so, and hears the block a press named while it
// was loading.
self.addEventListener("message", (event) => {
    if (event.data?.ready && pending) {
        if (Date.now() - pending.at < OPEN_WITHIN) {
            event.source?.postMessage({ open: pending.open });
        }
        pending = null;
    }
});

// The push service replaced this browser's subscription: subscribe again with the same
// server key, and tell the windows, which register the new endpoint with the server
// (the page does so whenever it connects, too).
self.addEventListener("pushsubscriptionchange", (event) => {
    event.waitUntil(
        (async () => {
            try {
                const key = event.oldSubscription?.options?.applicationServerKey;
                if (key) {
                    await self.registration.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key });
                }
            } catch (error) {
                console.error("the push subscription could not be renewed", error);
            }
            for (const client of await windows()) {
                client.postMessage({ changed: true });
            }
        })(),
    );
});
