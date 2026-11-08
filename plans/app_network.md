# Network for apps

Depends on: none.

Apps get the network a browser has: `fetch` and WebSocket. Raw sockets and
WebRTC are deferred (plans/deferred/app_network_sockets.md). Every app is
first-party and trusted today, so access is wide open; the manifest stays the
place to restrict it later. Not implemented yet.

## What exists

- `fetch(url)` in `crates/be-sdk-api/wit/host-services.wit` is a GET that
  answers the body bytes, refused unless the manifest's `network` names the
  URL's host (`allowed` in `be-host-runtime/src/instances/services.rs`).
- The host side is `be-host-base/src/platform/http/`: ureq on native and
  Android, the browser's fetch on the web, a 32 MiB body limit
  (`MAX_BODY_BYTES`). Only be-app-map uses it, for `vector.openstreetmap.org`.
- In the browser an app runs in a worker, and every guest call is an async op
  the host answers (decisions/guest_calls_are_async_ops_except_local_ones.md).
  WIT already carries `stream<T>` (`blocks.wit`'s `replica`).
- be-block-client already links tokio-tungstenite and rustls.
- There is no call to open a URL in the system browser.

## The design

A new `network` interface in `crates/be-sdk-api/wit` replaces
`host-services` `fetch`. Its shapes follow what browsers offer, since the web
is the narrowest platform.

- **`fetch(request)`**: a method, URL, headers and body; it answers the status
  and headers, with the body as a `stream<list<u8>>`, so no body limit applies.
  CORS still applies on the web. be-app-map moves to it, and `fetch(url)` goes.
- **WebSocket**: a resource opened with a URL and subprotocols, with `send`
  (text or binary), a stream of messages that ends on close, and `close`. No
  custom headers anywhere, because browsers can't set them; services that need
  auth on a socket take a token got with `fetch`.
- **`open-url`**: opens a link in the system browser, or a new tab on the web.
- **Credentials are the app's.** The host adds no tokens and knows no outside
  service; each app keeps its own (plans/owner_settings.md) and handles 401s,
  expiry and rotation by its service's rules.
- **The manifest.** `network` keeps its host list and also takes `"*"`, which
  first-party apps use for now.
  Restricting third-party apps later is a manifest change, not an API change.
- **The SDK.** `be_sdk` gets async Rust wrappers for each resource, and
  `be-sdk-test` records each request as a `HostCall` with the whole request, and
  answers with a response, a socket's messages or its close.

## Order of work

1. `fetch(request)`, `"*"` and `open-url`; move be-app-map over. Update
   guides/apps/host_services.md.
2. WebSocket.

## Not planned here

- Restricting what third-party apps may reach: that comes with app trust
  (VISION.md, Extension).
- Relaying any service through be-server to get around CORS.
