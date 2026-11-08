# WebSocket, raw sockets and WebRTC for apps

Depends on: plans/app_network.md.

The rest of the network interface (plans/app_network.md), set aside until an
app needs it.

- **WebSocket**: a resource opened with a URL and subprotocols, with `send`
  (text or binary), a stream of messages that ends on close, and `close`. No
  custom headers anywhere, because browsers can't set them; services that need
  auth on a socket take a token got with `fetch`.
- **TCP and UDP**: `tcp-socket` (connect to host and port, optionally with TLS
  done by the host with rustls and the platform's roots; a read stream, write,
  close) and `udp-socket`. Native and Android only; the web answers
  `unavailable`.
- **WebRTC**: data channels, with signalling over block peer messages
  (guides/blocks/apps.md), so apps need no signalling server. It belongs with
  VISION's layer 2 (connection setup) and is designed with it.
- **The manifest.** Sockets need `"sockets": true` beside `network`.

## Order of work

1. WebSocket, when the first app needs it.
2. TCP and UDP, when the first app needs them.
3. WebRTC data channels, with layer 2.
