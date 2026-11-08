# Network for apps

Depends on: none.

Apps get `fetch` with full requests and streamed bodies, and can open a URL.
WebSocket, raw sockets and WebRTC are deferred
(plans/deferred/app_network_sockets.md). Every app is first-party and trusted
today, so access is wide open; the manifest stays the place to restrict it
later. Not implemented yet.

## What exists

- `fetch(url)` in `crates/be-sdk-api/wit/host-services.wit` is a GET that
  answers the body bytes, refused unless the manifest's `network` names the
  URL's host (`allowed` in `be-host-runtime/src/instances.rs`, called from
  `instances/services.rs`), and refused for anything but `https://`.
- The host side is `be-host-base/src/platform/http/`: ureq on native and
  Android, the browser's fetch on the web, a 32 MiB body limit
  (`MAX_BODY_BYTES`). Only be-app-map uses it, for `vector.openstreetmap.org`.
- In the browser an app runs in a worker, and every guest call is an async op
  the host answers (decisions/guest_calls_are_async_ops_except_local_ones.md).
  WIT already carries `stream<T>` (`blocks.wit`'s `replica`).
- The host opens URLs itself with `be_host_base::platform::browser::open_browser`
  (native, Android, and `window.open` on the web), but apps have no call for it.

## The design

A new `network` interface in `crates/be-sdk-api/wit` replaces
`host-services` `fetch`. Its shapes follow what browsers offer, since the web
is the narrowest platform.

- **`fetch(request)`**: a method, URL, headers and body; it answers the status
  and headers, with the body as a `stream<list<u8>>`, so no body limit applies.
  CORS still applies on the web. be-app-map moves to it, and `fetch(url)` goes.
  Natively the host's HTTP is blocking ureq on a thread; a streamed body needs
  that thread to send chunks to the guest as they arrive (or an async client),
  and the design goes in the PR.
- **`open-url`**: opens a link in the system browser, or a new tab on the web,
  through `open_browser`. On the web, `window.open` from an async op answered
  after the click has lost the click's user activation and is blocked as a
  popup, so the web host opens the tab while it handles the press when it
  can, and otherwise shows a link the person presses. Checked by hand in a
  browser; tests check the call is made.
- **Credentials are the app's.** The host adds no tokens and knows no outside
  service; each app keeps its own in an owner-only settings kind (guides/settings.md, Owner only) and handles 401s,
  expiry and rotation by its service's rules.
- **The manifest.** `network` keeps its host list and also takes `"*"`, which
  first-party apps use for now. Both keep the `https://` rule; host tests
  answer requests with `be-sdk-test`'s `HostCall` doubles, so they never need
  an `http://127.0.0.1` fake. Restricting third-party apps later is a manifest change, not an API change.
- **The SDK.** `be_sdk` gets async Rust wrappers for each resource, and
  `be-sdk-test` records each request as a `HostCall` with the whole request, and
  answers with a response whose body arrives in chunks.

## Order of work

1. `fetch(request)`, `"*"` and `open-url`; move be-app-map over. Update
   guides/apps/host_services.md and the `network` line in
   guides/adding_an_app.md. Done: be-app-map's tests pass on the new call,
   `fetch(url)` is deleted, a `be-sdk-test` test streams a body in chunks, and
   one checks `"*"` still refuses `http://`.

## Not planned here

- Restricting what third-party apps may reach: that comes with app trust
  (VISION.md, Extension).
- Relaying any service through be-server to get around CORS.
