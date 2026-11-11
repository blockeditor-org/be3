# Notifications: decrypting with no tab open

Depends on: plans/notifications.md.

On the web, a push that arrives with no tab open shows "New message" naming
nothing (plans/notifications.md). Decrypting it in the service worker needs:

- a SQLite reader for the refs database in the service worker's own wasm,
  and the key-unwrapping code;
- the identity secret, which lives in the separate host-state database;
- a way to read both while no page holds them, and to never conflict with a
  page that opens meanwhile (the page takes a Web Lock and exclusive
  `FileSystemSyncAccessHandle`s, which a service worker cannot create;
  `crates/be-host/web/files.js`).

## Hiding a notification's text on the lock screen

A shown notification carries its sender's and block's names and its text in
plaintext, readable by whoever holds a locked device. A this-device setting
could show only "New message" there, on the web (sw.js) and on Android.
