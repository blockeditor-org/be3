// The worker push-web-test registers: the real sw.js, with showing a notification reported to
// the page, since a headless browser never grants the permission to show one.
self.registration.showNotification = async (title, options = {}) => {
    const notification = { title, ...options };
    for (const client of await self.clients.matchAll({ includeUncontrolled: true })) {
        client.postMessage({ shown: notification });
    }
};

importScripts("sw-under-test.js");
