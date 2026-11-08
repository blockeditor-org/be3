# Notifications

Depends on: none.

Push notifications for people who aren't connected, as a host feature any
block type can use; chat (plans/chat.md) is the first. Not implemented yet.

## What exists

- No push code at all: no Web Push, FCM, UnifiedPush or APNs.
- The web build has a service worker (`crates/be-host/web/sw.js`), which only
  caches the host's files.
- A device keeps key rows in the refs database (SQLite, in the origin private
  file system on the web) and unwraps them offline (`Peer::cached`,
  guides/blocks/client.md).

## The design

- **Sending.** An app seals a short notification (block name, author, preview;
  at most about 1 KiB) under the block's data key and calls
  `notify(block, sealed, mentions)`, a host call on the app protocol with no
  framework types. The host passes it to the server.
- **The server** pushes it to the block's members who aren't connected,
  skipping those who muted the block. It stores each account's mutes (per
  block, and a mentions-only mode) and learns who was mentioned; it never sees
  the text.
- **Two senders.** A device registers an endpoint of one kind:
  - **Web Push** (RFC 8030, payloads encrypted with RFC 8291, VAPID,
    RFC 8292): the web's service worker subscription, and Android through
    UnifiedPush, whose distributors take Web Push payloads. The server makes
    its VAPID key pair on first start and keeps it in its data directory.
  - **FCM** (HTTP v1 API) for Android devices with Google Play services and
    no UnifiedPush distributor. The server signs in with a service account
    (`--fcm-credentials`, a file; a server without it offers only Web Push),
    and the APK carries the Firebase project's client config. The payload is
    the same sealed notification, sent as a data message.
  - The Android host prefers a UnifiedPush distributor when one is installed
    and falls back to FCM. Record the choice as a decision (Web Push
    everywhere, UnifiedPush or FCM on Android).
- **Devices** register and unregister endpoints with the server per account
  and organization, and the server drops an endpoint the push service answers
  404 or 410 for.
- **Receiving.** The sealed notification names its key id (sealed bytes name
  their key), so the device picks the right epoch, unwraps it from its cached
  key rows, decrypts and shows the notification: no block load, no type
  module, no network. Missing a key, it shows a generic "New message" naming
  nothing. Pressing it opens the block.
- **On the web** the service worker cannot open the refs or host-state
  databases: the page holds a Web Lock and exclusive sync handles on them
  (`crates/be-host/web/files.js`), and the identity secret is in host state.
  With a tab open, the service worker posts the push to it, the tab decrypts
  and answers, and the worker shows that; with none, it shows "New message"
  naming nothing. Decrypting with no tab open is deferred
  (plans/deferred/notifications.md).
- **On Android** pushes arrive with the app closed, so a service receives them
  and loads the Rust library without the activity. UnifiedPush works with
  plain intents; the FCM client (firebase-messaging, Play services) is an AAR
  from Maven, which the buck Android build cannot take yet
  (`buck/android/defs.bzl` takes Java sources only).
- **Limits.** An Android high-priority message has about 10–20 s; every web
  push must show a notification, so the service worker always shows one.

## Order of work

1. Endpoints, mutes and `notify` on the server, sending Web Push with VAPID.
   Done: server tests deliver an encrypted push to a test endpoint for a member
   who isn't connected, none to a connected or muted one, and drop an endpoint
   answered 410.
2. The `notify` host call and its `be-sdk-test` double. Done: a `BeuiTest`
   records the call.
3. The web: subscribing, the service worker handing the push to an open tab
   and showing what it answers, opening the block. Done: a Chromium test
   delivers a push to the service worker (DevTools
   `ServiceWorker.deliverPushMessage`): with a tab open the notification shows
   the decrypted text; with none, or without the key, "New message".
4. Android through UnifiedPush, with the receiving service. Done: on the
   emulator, with a test distributor app and the app closed, a push shows the
   decrypted notification and opens the block.
5. AAR and Maven dependencies in the buck Android build. Done: an APK builds
   with one AAR dependency and runs on the emulator.
6. FCM on the server and Android. Done unattended: a server test sends to a
   fake FCM endpoint with a test service account and checks the request, and
   the APK builds with the FCM client. Real delivery is blocked on the user's
   Firebase project and its client config, which only the user can supply.
7. Mute settings in the block menu, with a painting.

## Not planned here

- iOS: there is no iOS host. Its notification service extension (about 30 s,
  little memory, keys from the keychain) is designed when one exists.
