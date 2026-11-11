// The page's side of push notifications (plans/notifications.md, sw.js is the
// worker's): subscribing this browser, answering the worker's pushes and
// opening the block a notification names.
//
// The page does not know the host's calls: host.js binds them with `bindPush`,
// since the module that answers them is instantiated there.

// Answers each push the worker posts (a PushMessage's bytes and a port) with
// what `open(bytes)` resolves to, or null when it cannot open one; hands
// `openBlock({ organization, block })` the block a pressed notification names, and calls
// `changed()` when the push service replaced this browser's subscription.
export function bindPush({ open, openBlock, changed }) {
    if (!navigator.serviceWorker) {
        return;
    }
    navigator.serviceWorker.addEventListener("message", (event) => {
        const { push, open: named, changed: replaced } = event.data ?? {};
        if (push instanceof Uint8Array && event.ports[0]) {
            const port = event.ports[0];
            open(push).then(
                (answer) => port.postMessage(answer ?? null),
                () => port.postMessage(null),
            );
        } else if (named?.block) {
            openBlock(named);
        } else if (replaced) {
            changed();
        }
    });
    // A press may have named a block while this page was loading.
    navigator.serviceWorker.ready.then((registration) => registration.active?.postMessage({ ready: true }));
}

// "granted", "denied" or "default" (not asked yet), or "unsupported".
export function pushPermission() {
    if (!("Notification" in globalThis) || !("PushManager" in globalThis) || !navigator.serviceWorker) {
        return "unsupported";
    }
    return Notification.permission;
}

async function registration() {
    const found = await navigator.serviceWorker.getRegistration();
    if (!found) {
        throw new Error("the service worker is not registered, so this browser cannot receive pushes");
    }
    return found;
}

function sameKey(subscription, key) {
    const held = subscription.options?.applicationServerKey;
    if (!held) {
        return false;
    }
    const bytes = new Uint8Array(held);
    return bytes.length === key.length && bytes.every((byte, index) => byte === key[index]);
}

// Subscribes this browser to pushes from the server whose VAPID public key is
// `serverKey` (bytes), asking permission first: call it from a person's gesture.
// Resolves to { address, p256dh, auth, replaced } for the server's `register-endpoint`:
// `replaced` is the address of a subscription made for another key, which this dropped and
// the server should forget.
export async function subscribePush(serverKey) {
    if (pushPermission() === "unsupported") {
        throw new Error("this browser cannot show notifications");
    }
    if ((await Notification.requestPermission()) !== "granted") {
        throw new Error("notifications are not allowed for this site");
    }
    const worker = await registration();
    let replaced = null;
    const existing = await worker.pushManager.getSubscription();
    if (existing && !sameKey(existing, serverKey)) {
        // Another server, or this one with a new key: a subscription cannot be made again
        // under a different key until the old one is gone.
        replaced = existing.endpoint;
        await existing.unsubscribe();
    }
    const subscription = await worker.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: serverKey,
    });
    return { ...described(subscription), replaced };
}

function described(subscription) {
    return {
        address: subscription.endpoint,
        p256dh: new Uint8Array(subscription.getKey("p256dh")),
        auth: new Uint8Array(subscription.getKey("auth")),
    };
}

// This browser's subscription as `register-endpoint` takes it, or null when it has none.
export async function currentPush() {
    if (pushPermission() !== "granted") {
        return null;
    }
    const subscription = await (await registration()).pushManager.getSubscription();
    return subscription ? described(subscription) : null;
}

// "unsupported", "denied", "on" (subscribed) or "off".
export async function pushState() {
    const permission = pushPermission();
    if (permission === "unsupported" || permission === "denied") {
        return permission;
    }
    return (await currentPush()) ? "on" : "off";
}

// Drops this browser's subscription, resolving to its address, or null when it
// had none.
export async function unsubscribePush() {
    if (pushPermission() === "unsupported") {
        return null;
    }
    const subscription = await (await registration()).pushManager.getSubscription();
    if (!subscription) {
        return null;
    }
    await subscription.unsubscribe();
    // What the worker kept of the pushes it showed (sw.js) goes with the subscription.
    indexedDB.deleteDatabase("be-push");
    return subscription.endpoint;
}
