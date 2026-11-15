# Android takes pushes through UnifiedPush

Push notifications are Web Push everywhere (RFC 8030, VAPID by RFC 8292, payloads encrypted
by RFC 8291), which be-server already sends. On Android be-host takes them through a
UnifiedPush distributor, an app the person installs (Sunup, ntfy, or another), talking to it
with plain intents (`crates/be-host/android/java/com/be3/host/Push.java`): the distributor's
endpoint is a Web Push URL, so the server registers it as `PushService::WebPush` and posts to
it as it posts to a browser's, and the distributor hands the app the encrypted body, which
be-host decrypts itself with keys it keeps (`be_host_stack::device_push`). The server does not
change.

The endpoints still pass the server's allowlist (decisions/push_endpoints_are_allowlisted.md):
Sunup's are on Mozilla autopush (`*.push.services.mozilla.com`), allowed already; ntfy's, on
ntfy.sh or a self-hosted ntfy, need the operator's `--push-host` (guides/hosting.md). A device
with no distributor installed shows notifications as unsupported.

## Why

- UnifiedPush needs nothing but intents and the Web Push the server already speaks, so it builds
  with the buck Android build as it is, which takes Java sources only (`buck/android/defs.bzl`).
- The payload stays end to end: the distributor and its server see only RFC 8291 ciphertext of
  text that is itself sealed under the block's key.

## Rejected, for now

- FCM (Firebase Cloud Messaging), which every device with Play services has: its client is an
  AAR from Maven, which the buck Android build cannot take yet, and it needs a Firebase project.
  It is deferred until the build takes AARs (plans/deferred/notifications.md); the server
  already sends to FCM (`--fcm-credentials`).
- The UnifiedPush connector library: an AAR too, and the protocol it wraps is a handful of
  intents.
