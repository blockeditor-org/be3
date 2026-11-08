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
    UnifiedPush, whose distributors take Web Push payloads.
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
- **Receiving.** The device unwraps the block's key from its cached key rows,
  decrypts and shows the notification: no block load, no type module, no
  network. Missing a key, it shows a generic "New message" naming nothing.
  Pressing it opens the block. On the web this runs in the service worker,
  which reads the refs database itself.
- **Limits.** An Android high-priority message has about 10–20 s; every web
  push must show a notification, so the service worker always shows one.

## Order of work

1. Endpoints, mutes and `notify` on the server, sending Web Push with VAPID.
   Done: server tests deliver an encrypted push to a test endpoint for a member
   who isn't connected, none to a connected or muted one, and drop an endpoint
   answered 410.
2. The `notify` host call and its `be-sdk-test` double. Done: a `BeuiTest`
   records the call.
3. The web: subscribing, the service worker decrypting and showing it, opening
   the block. Done: a Chromium test delivers a push to the service worker
   (DevTools `ServiceWorker.deliverPushMessage`) and the notification shows the
   decrypted text, or "New message" without the key.
4. Android through UnifiedPush. Done: on the emulator, with a test distributor
   app, a push shows the decrypted notification and opens the block.
5. FCM on the server and Android. Done: a server test sends to a fake FCM
   endpoint with a test service account and checks the request; on an
   emulator image with Google Play services and no distributor, the host
   registers with FCM (a manual check, since delivery needs a real Firebase
   project).
6. Mute settings in the block menu, with a painting.

## Not planned here

- iOS: there is no iOS host. Its notification service extension (about 30 s,
  little memory, keys from the keychain) is designed when one exists.
