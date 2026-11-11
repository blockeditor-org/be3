// The worker push-web-test registers: the real sw.js, with three changes a headless browser
// needs. Showing a notification is reported to the page, since the browser never grants the
// permission to show one; the clock can be moved on, to see a shown push forgotten; and the
// page can raise the events a person's press and the push service would, which DevTools
// cannot deliver.
let offset = 0;
const now = Date.now.bind(Date);
Date.now = () => now() + offset;

self.registration.showNotification = async (title, options = {}) => {
    for (const client of await self.clients.matchAll({ includeUncontrolled: true })) {
        client.postMessage({ shown: { title, ...options } });
    }
};

self.addEventListener("message", (event) => {
    const { advance, click, changed } = event.data ?? {};
    if (advance) {
        offset += advance;
    }
    if (click) {
        self.dispatchEvent(Object.assign(new ExtendableEvent("notificationclick"), { notification: { close() {}, data: click } }));
    }
    if (changed) {
        self.dispatchEvent(new ExtendableEvent("pushsubscriptionchange"));
    }
});

importScripts("sw-under-test.js");
