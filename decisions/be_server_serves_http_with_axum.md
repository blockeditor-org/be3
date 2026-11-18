# be-server serves HTTP with hyper, tower and axum

be-server parses no HTTP itself. The accept loop admits a connection by its peer's address and
hyper's HTTP/1 connection serves it (`http::serve`); a front service counts each request against
its client's places, then axum handlers answer it, with tower-http's body limits, body
timeouts and response headers. The S3 endpoint is s3s's own tower service; a websocket upgrade
is hyper's, and the upgraded connection goes to tokio-tungstenite, as before.

## Why

The hand-written server (httparse heads, bodies framed by hand, responses formatted as strings)
was about 1,200 lines that had to get framing, keep-alive, smuggling and timeouts right on its
own, and S3 requests were parsed twice, once by it and again as `http` types for s3s. hyper's
parser is the one most of the Rust ecosystem relies on: it refuses two differing lengths and any
length that is not digits, and frames every body the way a proxy in front does. axum is the
usual way to write handlers on hyper and tower, and s3s is a tower service already, so the
endpoint plugs in as it is. The user asked for axum.

The per-address and not-yet-signed-in limits stay in the accept loop and the front service, so
they apply before a body is read. Routes are chosen by the end of the path, not by axum's
router, since a proxy may mount the server below any path. A request with a
`Transfer-Encoding` is still refused, so no proxy in front can frame a body otherwise than the
server. Two `Content-Length`s with the same value, which the old parser refused, are now
accepted as hyper and RFC 9112 accept them: every reader frames that body the same way.

A trusted proxy pools its connections, so the front service takes a connection's places for
the client each request forwards, and gives them back when the next request is another
client's.

## Rejected

- axum's own websockets (`ws`): they hide the stream, so the socket could no longer note each
  read (`Watched`, `next_heard`), and they would bring a second tungstenite beside
  be-block-client's.
- `axum::serve`: it accepts connections itself, so the limits taken in the accept loop could
  not be applied before a request is read.
- Keeping the hand-written parser.
