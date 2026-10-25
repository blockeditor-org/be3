// The service worker that opens the app without a network.
//
// Every request of the page's own origin, apart from the server's (/api and
// /auth), goes to the network first, so an online page always gets what is
// deployed, and each answer is kept. When the network fails, the kept answer
// is served instead: the page, the module, the shims and every plugin the app
// loaded before. A kept answer carries its headers, so the page is still
// cross-origin isolated when it comes from here.
//
// The page registers this at the root of its address (block.js), which is
// where it has to be to answer for the page itself: the bundle's root here,
// and /sw.js beside the entry point when deployed (crates/local-tools deploy).

const CACHE = "block-app";

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
