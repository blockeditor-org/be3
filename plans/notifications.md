# Notifications: what is left

Depends on: none.

Push notifications for people who aren't connected, as a host feature any
block type can use; chat is the first. Built: the server side
(guides/blocks/server.md, "Notifications"), opening a push on a device
(guides/blocks/client.md, the refs database), sending
(guides/type_modules.md, "Announcements"), the host call and the mute menu
(guides/apps/host_services.md), and the web build's service worker,
subscribing and opening the block (guides/running/web.md). No Android device
takes a push yet.

## The design left

- **Receiving** is built on the web and left for Android. On the web
  (`be-host/src/web_push.rs`, `web/sw.js`) a device names the sender from its
  contacts and the block from its graph, never from what the sender wrote, and
  missing a key shows a generic "New message" naming nothing; it drops a push it
  already showed (the server can resend one within
  `MAX_NOTIFICATION_AGE_MILLISECONDS`); pressing one opens the block; and it
  has the server forget its endpoint when its account signs out (`web_push::sign_out`).
  Android's receiving service does the same.
- **The lock screen** shows a notification's text in plaintext, whoever holds
  the device. A setting to show only "New message" there is a follow-up
  (plans/deferred/notifications.md).
- **On Android** pushes arrive with the app closed, so a service receives them
  and loads the Rust library without the activity, decrypting RFC 8291 itself
  with be-lib-web-push. UnifiedPush works with plain intents; the FCM client
  (firebase-messaging, Play services) is an AAR from Maven, which the buck
  Android build cannot take yet (`buck/android/defs.bzl` takes Java sources
  only). The Android host prefers a UnifiedPush distributor when one is
  installed and falls back to FCM (set aside, plans/deferred/notifications.md). Record the choice as a decision (Web Push
  everywhere, UnifiedPush or FCM on Android).
- **Limits.** An Android high-priority message has about 10–20 s; every web
  push must show a notification, so the service worker always shows one.

## Order of work

1. Android through UnifiedPush, with the receiving service. Done: on the
   emulator, with a test distributor app and the app closed, a push shows the
   decrypted notification and opens the block. Signing out unregisters the
   endpoint.

## Not planned here

- iOS: there is no iOS host. Its notification service extension (about 30 s,
  little memory, keys from the keychain) is designed when one exists.
