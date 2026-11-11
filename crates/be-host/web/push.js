// The page's side of push notifications (plans/notifications.md, sw.js is the
// worker's): subscribing this browser, answering the worker's pushes and
// opening the block a notification names.
//
// The page does not know the host's calls: host.js binds them with `bindPush`,
// since the module that answers them is instantiated there.

// Answers each push the worker posts (a PushMessage's bytes and a port) with
// what `open(bytes)` resolves to, or null when it cannot open one, and hands
// `openBlock({ organization, block })` the block a pressed notification names.
export function bindPush({ open, openBlock }) {
    if (!navigator.serviceWorker) {
        return;
    }
    navigator.serviceWorker.addEventListener("message", (event) => {
        const { push, open: named } = event.data ?? {};
        if (push instanceof Uint8Array && event.ports[0]) {
            const port = event.ports[0];
            open(push).then(
                (answer) => port.postMessage(answer ?? null),
                () => port.postMessage(null),
            );
        } else if (named?.block) {
            openBlock(named);
        }
    });
}

// "granted", "denied" or "default" (not asked yet), or "unsupported".
export function pushPermission() {
    if (!("Notification" in globalThis) || !navigator.serviceWorker) {
        return "unsupported";
    }
    return Notification.permission;
}

// Subscribes this browser to pushes from the server whose VAPID public key is
// `serverKey` (bytes), asking permission first: call it from a person's gesture.
// Resolves to { address, p256dh, auth } for the server's `register-endpoint`.
export async function subscribePush(serverKey) {
    if (pushPermission() === "unsupported") {
        throw new Error("this browser cannot show notifications");
    }
    if ((await Notification.requestPermission()) !== "granted") {
        throw new Error("notifications are not allowed for this site");
    }
    const registration = await navigator.serviceWorker.ready;
    const subscription = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: serverKey,
    });
    return {
        address: subscription.endpoint,
        p256dh: new Uint8Array(subscription.getKey("p256dh")),
        auth: new Uint8Array(subscription.getKey("auth")),
    };
}

// Drops this browser's subscription, resolving to its address, or null when it
// had none.
export async function unsubscribePush() {
    if (pushPermission() === "unsupported") {
        return null;
    }
    const registration = await navigator.serviceWorker.ready;
    const subscription = await registration.pushManager.getSubscription();
    if (!subscription) {
        return null;
    }
    await subscription.unsubscribe();
    return subscription.endpoint;
}
