# Raw sockets and WebRTC for apps

Depends on: plans/app_network.md.

The rest of the network interface (plans/app_network.md), set aside until an
app needs it.

- **TCP and UDP**: `tcp-socket` (connect to host and port, optionally with TLS
  done by the host with rustls and the platform's roots; a read stream, write,
  close) and `udp-socket`. Native and Android only; the web answers
  `unavailable`.
- **WebRTC**: data channels, with signalling over block peer messages
  (guides/blocks/apps.md), so apps need no signalling server. It belongs with
  VISION's layer 2 (connection setup) and is designed with it.
- **The manifest.** Sockets need `"sockets": true` beside `network`.

## Order of work

1. TCP and UDP, when the first app needs them.
2. WebRTC data channels, with layer 2.
