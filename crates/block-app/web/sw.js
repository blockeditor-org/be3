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

async function answer(request) {
    const cache = await caches.open(CACHE);
    try {
        // Revalidated, since a file of the bundle's root keeps its name across
        // builds, and the browser's cache would otherwise mix two builds'
        // module and glue. A deployed build's own files are under /data/BUILD
        // and never change.
        const immutable = new URL(request.url).pathname.startsWith("/data/");
        const response = await fetch(request, immutable ? {} : { cache: "no-cache" });
        if (response.ok && response.type === "basic") {
            await cache.put(request, response.clone());
        }
        return response;
    } catch (error) {
        const found = await cache.match(request, { ignoreVary: true });
        if (found) {
            return found;
        }
        // An address the page was opened at with other parameters still
        // finds the page.
        if (request.mode === "navigate") {
            const page = await cache.match(request, { ignoreSearch: true, ignoreVary: true });
            if (page) {
                return page;
            }
        }
        throw error;
    }
}

self.addEventListener("fetch", (event) => {
    if (kept(event.request)) {
        event.respondWith(answer(event.request));
    }
});
