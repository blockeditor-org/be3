# Notifications

Depends on: none.

Push notifications for people who aren't connected, as a host feature any
block type can use; chat (plans/chat.md) is the first. The server side, the
host call and the mute menu have landed (guides/blocks/server.md,
"Notifications"; guides/apps/host_services.md); no device takes a push yet.

## What exists

- **The server** keeps each account's endpoints per organization (Web Push or
  FCM, with the device's RFC 8291 keys), its mode per block (`all`,
  `mentions`, `off`) and a VAPID key, and pushes `notify`'s sealed bytes to the
  block's viewers who have no connection open, encrypted to each device and
  padded to one length (guides/blocks/server.md, "Notifications"). FCM sends a
  data message (`data.push`, the same encrypted bytes, base64url) once the
  server has `--fcm-credentials`; no Firebase project exists yet.
- **The push** a device gets decrypts with `be_lib_web_push::open` (its own
  keys) to a `be_block_protocol::PushMessage`: organization, block, the sender
  the server vouches for, then the text sealed under the block's data key with
  the block and the time it was sent. `Peer::open_notification` opens that
  offline, from the key rows the device keeps in its refs database
  (`Peer::cached`, guides/blocks/client.md), refusing one sealed about another
  block or older than `MAX_NOTIFICATION_AGE_MILLISECONDS`.
- **The app protocol**: `host-services.notify(block, text, mentions)`, which
  the host seals (`Peer::notify`); `NotifyModesWatch` and `set-notify-mode`,
  which the workspace's block menu uses.
- Nothing registers an endpoint: no device subscribes yet
  (`Peer::push_key`, `register_endpoint` and `unregister_endpoint` wait for
  step 1).
- The web build has a service worker (`crates/be-host/web/sw.js`), which only
  caches the host's files.

## The design

- **Receiving.** The sealed notification names its key id (sealed bytes name
  their key), so the device picks the right epoch, unwraps it from its cached
  key rows, decrypts and shows the notification (`Peer::open_notification`):
  no block load, no type module, no network. It names the sender from its
  contacts and the block from its graph, never from what the sender wrote.
  Missing a key, it shows a generic "New message" naming nothing. Pressing it
  opens the block. A device unregisters its endpoint when its account signs
  out.
- **On the web** the service worker cannot open the refs or host-state
  databases: the page holds a Web Lock and exclusive sync handles on them
  (`crates/be-host/web/files.js`), and the identity secret is in host state.
  With a tab open, the service worker posts the push to it, the tab decrypts
  and answers, and the worker shows that; with none, it shows "New message"
  naming nothing. Decrypting with no tab open is deferred
  (plans/deferred/notifications.md). Subscribing asks the browser's
  permission, so it waits for a person's gesture (a setting on this device).
- **On Android** pushes arrive with the app closed, so a service receives them
  and loads the Rust library without the activity, decrypting RFC 8291 itself
  with be-lib-web-push. UnifiedPush works with
  plain intents; the FCM client (firebase-messaging, Play services) is an AAR
  from Maven, which the buck Android build cannot take yet
  (`buck/android/defs.bzl` takes Java sources only). The Android host prefers a
  UnifiedPush distributor when one is installed and falls back to FCM. Record
  the choice as a decision (Web Push everywhere, UnifiedPush or FCM on
  Android).
- **Limits.** An Android high-priority message has about 10–20 s; every web
  push must show a notification, so the service worker always shows one.

## Order of work

1. The web: subscribing, the service worker handing the push to an open tab
   and showing what it answers, opening the block. Done: a Chromium test
   delivers a push to the service worker (DevTools
   `ServiceWorker.deliverPushMessage`): with a tab open the notification shows
   the decrypted text under the sender's and block's names; with none, or
   without the key, "New message". Signing out unregisters the endpoint.
2. Android through UnifiedPush, with the receiving service. Done: on the
   emulator, with a test distributor app and the app closed, a push shows the
   decrypted notification and opens the block. Signing out unregisters the
   endpoint.
3. AAR and Maven dependencies in the buck Android build. Done: an APK builds
   with one AAR dependency and runs on the emulator.
4. FCM on Android. Done unattended: the APK builds with the FCM client, which
   registers only through UnifiedPush without a client config. FCM stays off
   until the user creates a Firebase project, starts the server with its
   service account (`--fcm-credentials`) and gives the APK its client config
   (`google-services.json`).

## Not planned here

- iOS: there is no iOS host. Its notification service extension (about 30 s,
  little memory, keys from the keychain) is designed when one exists.
