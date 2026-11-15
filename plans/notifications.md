# Notifications: what is left

Depends on: none.

Push notifications for people who aren't connected, as a host feature any
block type can use; chat is the first. Built: the server side
(guides/blocks/server.md, "Notifications"), opening a push on a device
(guides/blocks/client.md, the refs database), sending
(guides/type_modules.md, "Announcements"), the host call and the mute menu
(guides/apps/host_services.md), the web build's service worker, subscribing
and opening the block (guides/running/web.md), and Android through a
UnifiedPush distributor (decisions/android_push_via_unifiedpush.md,
guides/running/android.md, "Push notifications"), whose opening of a push with
the app closed is tested on Linux (be-host-stack's
`a_device_opens_a_push_from_its_refs_database_with_its_stack_stopped`).

## The design left

- **The lock screen** shows a notification's text in plaintext, whoever holds
  the device. A setting to show only "New message" there is a follow-up
  (plans/deferred/notifications.md).
- **FCM on Android**, for devices with no UnifiedPush distributor, waits for
  AAR dependencies in the buck Android build (plans/deferred/notifications.md).
- **Limits.** An Android high-priority message has about 10–20 s; every web
  push must show a notification, so the service worker always shows one.

## Order of work

1. Android on a device. Written without an emulator to run it (no KVM where it
   was built), so only its compiling is checked. Done: on the emulator, with a
   test distributor app (ntfy, with the server's `--push-host`) and the app
   closed, turning notifications on asks for the permission and registers an
   endpoint, a push shows the decrypted notification once, pressing it opens
   the block, and signing out of the last account unregisters the endpoint and
   the server forgets it. Fix what that finds in `android_push.rs` and the Java
   beside `MainActivity`.

## Not planned here

- iOS: there is no iOS host. Its notification service extension (about 30 s,
  little memory, keys from the keychain) is designed when one exists.
