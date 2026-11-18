//! be-server's HTTP, served with hyper, tower and axum: be-server parses no HTTP itself
//! (decisions/be_server_serves_http_with_axum.md). The accept loop (`serve_with_mail`) admits each
//! connection by its peer's address; [`serve`] serves it with hyper's HTTP/1 connection and
//! [`front`], which counts each request against its client's places before handing it to the
//! route its path names, each with its own body limit:
//! - `/auth…`: the account portal's pages and forms (`auth`);
//! - `…/storage/objects…`: the S3 endpoint (`s3`), s3s's own service;
//! - `POST …/call`: one call that needs no sign-in (`call`);
//! - `POST …/http`: the long-poll fallback, posts carrying frames (`posts`);
//! - anything else: a websocket upgrade, handed to tokio-tungstenite (`socket`).
//!
//! Paths are matched by their end, since the server is reached below whatever path a proxy
//! mounts it at.

use std::{
    collections::{HashMap, VecDeque},
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Extension, Router,
    body::{Body, Bytes},
    extract::{Request, State},
    response::{IntoResponse, Response},
};
use be_block_protocol::{MAX_FRAME_BYTES, Welcome, hello, read_hello};
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::{
    net::TcpStream,
    sync::{Mutex, Notify, OwnedSemaphorePermit, mpsc},
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        handshake::derive_accept_key,
        protocol::{Role, WebSocketConfig},
    },
};
use tower::ServiceExt;
use tower_http::{limit::RequestBodyLimitLayer, timeout::RequestBodyTimeoutLayer};

use crate::{
    Client, HANDSHAKE_TIMEOUT, IDLE_TIMEOUT, OUTBOUND_BYTES, Outbound, ServerError, Services,
    Watched,
    address::{Anonymous, Eviction, Held, PerAddress},
    greeting,
};

mod auth;
mod s3;

const PATH_SUFFIX: &str = "/http";
/// Where a one-shot call is posted: a greeting and one call that needs no sign-in, answered and
/// closed with no session (`one_shot`).
const CALL_SUFFIX: &str = "/call";
/// A one-shot call's body: a greeting and a call of a few short strings.
const MAX_CALL_BYTES: usize = 16 * 1024;
/// One-shot calls one address may make in each `CALLS_WINDOW`. Each sign-in is also polled no
/// faster than its interval.
pub(crate) const MAX_CALLS_PER_ADDRESS: u32 = 60;
pub(crate) const CALLS_WINDOW: Duration = Duration::from_secs(60);
const SESSION_HEADER: HeaderName = HeaderName::from_static("be3-session");
/// Set on an answer when the session has more to say, or frames of the post still to carry
/// out; the client posts again at once, with an empty body if it has nothing to send.
const MORE_HEADER: HeaderName = HeaderName::from_static("be3-more");
/// A request's head, its request line and headers, and how many headers it may have.
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;
const MAX_BODY_BYTES: usize = 2 * MAX_FRAME_BYTES;
/// Longest a request body may go without a byte arriving.
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// What a connection was let in with: its place among its address's connections and among
/// those not signed in, both taken at its first request for a trusted proxy's, what tells it it
/// was let go of to make room, and its place among the server's connections.
pub(crate) struct Admitted {
    pub(crate) place: Option<Held>,
    pub(crate) anonymous: Option<Anonymous>,
    pub(crate) eviction: Arc<Eviction>,
    pub(crate) slot: OwnedSemaphorePermit,
}

/// One accepted connection, as its requests see it.
struct Link {
    peer: SocketAddr,
    eviction: Arc<Eviction>,
    places: std::sync::Mutex<Places>,
    /// Its place among the server's connections, held by a websocket it becomes too.
    slot: Arc<OwnedSemaphorePermit>,
    /// Told at each request, so the first is waited for only `HANDSHAKE_TIMEOUT`.
    requested: Notify,
}

struct Places {
    /// The client the places are held for: the peer, or on a trusted proxy's connection the
    /// address its latest request forwarded.
    client: Option<IpAddr>,
    place: Option<Held>,
    anonymous: Option<Anonymous>,
    /// Whether a post on the connection signed its session in: its later requests no longer
    /// count among those not signed in.
    signed_in: bool,
}

/// What a route is handed with each request: the address it comes from and its connection.
#[derive(Clone)]
struct Admission {
    client: IpAddr,
    link: Arc<Link>,
}

/// The routes, built once and shared by every connection.
#[derive(Clone)]
pub(crate) struct Routes {
    auth: Router,
    call: Router,
    posts: Router,
    socket: Router,
    s3: Option<Router>,
}

impl Routes {
    pub(crate) fn new(services: &Services) -> Self {
        let limited = |router: Router<Services>, limit: usize| {
            router
                .layer(RequestBodyLimitLayer::new(limit))
                .layer(RequestBodyTimeoutLayer::new(BODY_READ_TIMEOUT))
                .with_state(services.clone())
        };
        Self {
            auth: limited(Router::new().fallback(auth::serve), auth::MAX_FORM_BYTES),
            call: limited(Router::new().fallback(call), MAX_CALL_BYTES),
            posts: limited(Router::new().fallback(posts), MAX_BODY_BYTES),
            socket: limited(Router::new().fallback(socket), 0),
            s3: services.s3.clone().map(s3::router),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    Auth,
    S3,
    Call,
    Posts,
    Socket,
}

impl Route {
    fn of(method: &Method, path: &str, s3: bool) -> Self {
        if s3 && s3::is_s3(path) {
            Self::S3
        } else if path == "/auth" || path.starts_with("/auth/") {
            Self::Auth
        } else if method == Method::POST && path.ends_with(CALL_SUFFIX) {
            Self::Call
        } else if method == Method::POST && path.ends_with(PATH_SUFFIX) {
            Self::Posts
        } else {
            Self::Socket
        }
    }
}

/// Serves one accepted connection until it closes, idles out, becomes a websocket, or is let go
/// of to make room for a newer one while it is not signed in.
pub(crate) async fn serve(
    stream: TcpStream,
    peer: SocketAddr,
    admitted: Admitted,
    routes: Routes,
    services: Services,
) -> Result<(), ServerError> {
    let Admitted {
        place,
        anonymous,
        eviction,
        slot,
    } = admitted;
    let link = Arc::new(Link {
        peer,
        eviction: Arc::clone(&eviction),
        places: std::sync::Mutex::new(Places {
            client: None,
            place,
            anonymous,
            signed_in: false,
        }),
        slot: Arc::new(slot),
        requested: Notify::new(),
    });
    let service = {
        let link = Arc::clone(&link);
        hyper::service::service_fn(move |request| {
            let (link, routes, services) = (Arc::clone(&link), routes.clone(), services.clone());
            async move { Ok::<_, Infallible>(front(request, link, &routes, &services).await) }
        })
    };
    // hyper's head timeout covers every head, the first and those after one is answered; the
    // first must also arrive within `HANDSHAKE_TIMEOUT`.
    let connection = hyper::server::conn::http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(IDLE_TIMEOUT)
        .max_header_size(MAX_HEAD_BYTES)
        .max_headers(MAX_HEADERS)
        .title_case_headers(true)
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades();
    let first = async {
        tokio::select! {
            () = link.requested.notified() => std::future::pending().await,
            () = tokio::time::sleep(HANDSHAKE_TIMEOUT) => {}
        }
    };
    tokio::select! {
        served = connection => served.map_err(|error| ServerError::Socket(error.to_string())),
        () = first => Err(crate::refused("the request never finished its head")),
        () = eviction.wait() => Ok(()),
    }
}

/// Counts a request against its client's places, as the accept loop counts a connection, and
/// hands it to its route.
async fn front(
    request: http::Request<hyper::body::Incoming>,
    link: Arc<Link>,
    routes: &Routes,
    services: &Services,
) -> Response {
    link.requested.notify_one();
    // hyper reads every length and refuses one that leaves doubt; a proxy in front that framed a
    // body by its Transfer-Encoding otherwise than hyper could smuggle a request in it, so none
    // is taken.
    if request.headers().contains_key(header::TRANSFER_ENCODING) {
        return closing(refuse(
            StatusCode::BAD_REQUEST,
            "a request has no Transfer-Encoding; send a Content-Length",
        ));
    }
    let client =
        services.client_address(link.peer.ip(), text(request.headers(), "x-forwarded-for"));
    let route = Route::of(request.method(), request.uri().path(), routes.s3.is_some());
    {
        let mut places = link.places.lock().unwrap();
        // A proxy speaks for many clients, and a pooled connection of its carries one client's
        // requests after another's: its places are taken for the address each request forwards,
        // and given back when the next is another client's.
        if places.client.is_some_and(|held| held != client) {
            *places = Places {
                client: None,
                place: None,
                anonymous: None,
                signed_in: false,
            };
        }
        places.client = Some(client);
        if places.place.is_none() {
            places.place = services.addresses.take(client);
        }
        let admitted = places.place.is_some()
            && match route {
                // An S3 request carries its own signature, which s3s checks, so it is not one of
                // the connections waiting to sign in: a client moves many objects at once.
                Route::S3 => {
                    places.anonymous = None;
                    true
                }
                _ if places.anonymous.is_some() || places.signed_in => true,
                _ => {
                    places.anonymous = services.unsigned.admit(client, &link.eviction);
                    places.anonymous.is_some()
                }
            };
        if !admitted {
            tracing::debug!(%client, "refusing a request: its address holds too many connections");
            return closing(refuse(
                StatusCode::TOO_MANY_REQUESTS,
                "too many connections from this address",
            ));
        }
    }
    let mut request = request.map(Body::new);
    request.extensions_mut().insert(Admission { client, link });
    let router = match route {
        Route::Auth => &routes.auth,
        Route::S3 => routes.s3.as_ref().expect("only routed with an S3 endpoint"),
        Route::Call => &routes.call,
        Route::Posts => &routes.posts,
        Route::Socket => &routes.socket,
    };
    match router.clone().oneshot(request).await {
        Ok(response) => response,
        Err(infallible) => match infallible {},
    }
}

/// A header's value as text, trimmed, if it is UTF-8.
fn text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    std::str::from_utf8(headers.get(name)?.as_bytes())
        .ok()
        .map(str::trim)
}

/// A plain-text refusal that says why.
fn refuse(status: StatusCode, message: &str) -> Response {
    (
        status,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        message.to_owned(),
    )
        .into_response()
}

/// `response`, after which the connection closes.
fn closing(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("close"));
    response
}

/// The answer to a request whose handling failed: a refusal says why, anything else (the
/// server's storage, its build, a socket) only that it failed.
fn failed(error: ServerError) -> Response {
    let status = match error {
        ServerError::Refused(..) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    refuse(status, &error.told())
}

/// A body read whole, within `within` if given; its route's layers cap its size and how long
/// it may stall.
async fn read(body: Body, within: Option<Duration>) -> Result<Bytes, Response> {
    let read = axum::body::to_bytes(body, usize::MAX);
    let read = match within {
        Some(within) => tokio::time::timeout(within, read)
            .await
            .unwrap_or_else(|_| Err(axum::Error::new("the body timed out"))),
        None => read.await,
    };
    read.map_err(|error| {
        tracing::debug!(%error, "a request body did not arrive");
        closing(refuse(
            StatusCode::BAD_REQUEST,
            "the request body did not arrive",
        ))
    })
}

/// Upgrades a request to a websocket, which serves the protocol (`serve_socket`) once the
/// answer is sent, holding the connection's places.
async fn socket(
    State(services): State<Services>,
    Extension(Admission { client, link }): Extension<Admission>,
    mut request: Request,
) -> Response {
    let headers = request.headers();
    let has = |name: header::HeaderName, token: &str| {
        headers.get_all(name).iter().any(|value| {
            value.to_str().is_ok_and(|value| {
                value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case(token))
            })
        })
    };
    let key = headers.get(header::SEC_WEBSOCKET_KEY).cloned();
    let (Some(key), true, true, true, true) = (
        key,
        request.method() == Method::GET,
        has(header::CONNECTION, "upgrade"),
        has(header::UPGRADE, "websocket"),
        has(header::SEC_WEBSOCKET_VERSION, "13"),
    ) else {
        return closing(refuse(
            StatusCode::BAD_REQUEST,
            "connect with a websocket, or post to …/http or …/call",
        ));
    };
    let authorization = text(headers, "authorization").map(str::to_owned);
    let (place, anonymous) = {
        let mut places = link.places.lock().unwrap();
        let anonymous = places
            .anonymous
            .take()
            .or_else(|| services.unsigned.admit(client, &link.eviction));
        (places.place.take(), anonymous)
    };
    let (Some(place), Some(anonymous)) = (place, anonymous) else {
        return closing(refuse(
            StatusCode::TOO_MANY_REQUESTS,
            "too many connections from this address",
        ));
    };
    let upgrade = hyper::upgrade::on(&mut request);
    let (eviction, slot) = (Arc::clone(&link.eviction), Arc::clone(&link.slot));
    tokio::spawn(async move {
        let _held = (place, slot);
        let upgraded = tokio::select! {
            upgraded = tokio::time::timeout(HANDSHAKE_TIMEOUT, upgrade) => upgraded,
            () = eviction.wait() => return,
        };
        let Ok(Ok(upgraded)) = upgraded else {
            tracing::debug!(%client, "a websocket upgrade never finished");
            return;
        };
        let (stream, heard) = Watched::new(TokioIo::new(upgraded));
        let socket = WebSocketStream::from_raw_socket(
            stream,
            Role::Server,
            Some(WebSocketConfig {
                max_frame_size: Some(MAX_FRAME_BYTES),
                max_message_size: Some(MAX_FRAME_BYTES),
                ..WebSocketConfig::default()
            }),
        )
        .await;
        tracing::debug!(%client, "connected");
        let served = crate::serve_socket(
            socket,
            &heard,
            &services,
            client,
            authorization,
            (anonymous, eviction),
        )
        .await;
        if let Err(error) = served {
            tracing::debug!(%client, %error, "a websocket ended with an error");
        }
    });
    (
        StatusCode::SWITCHING_PROTOCOLS,
        [
            (header::UPGRADE, HeaderValue::from_static("websocket")),
            (header::CONNECTION, HeaderValue::from_static("Upgrade")),
            (
                header::SEC_WEBSOCKET_ACCEPT,
                HeaderValue::from_str(&derive_accept_key(key.as_bytes()))
                    .expect("an accept key is base64"),
            ),
        ],
    )
        .into_response()
}

/// Serves a post of the long-poll fallback. Its connection counts as not signed in, and can be
/// let go of for a newer one, until a post it carries is for a session that is signed in.
async fn posts(
    State(services): State<Services>,
    Extension(Admission { client, link }): Extension<Admission>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let body = match read(body, None).await {
        Ok(body) => body,
        Err(refused) => return refused,
    };
    let answer = answer(
        text(&headers, SESSION_HEADER.as_str()),
        text(&headers, "authorization"),
        client,
        body,
        &services,
    )
    .await;
    match answer {
        Ok(answer) => {
            if answer.signed_in {
                let mut places = link.places.lock().unwrap();
                places.anonymous = None;
                places.signed_in = true;
            }
            answer.into_response()
        }
        Err(error) => closing(failed(error)),
    }
}

/// HTTP sessions open at once on the whole server, and for one client address.
const MAX_HTTP_SESSIONS: usize = 1024;
pub(crate) const MAX_HTTP_SESSIONS_PER_ADDRESS: usize = 32;

/// The HTTP sessions open, each signed in by the greeting that opened it.
pub(crate) struct Sessions {
    open: std::sync::Mutex<HashMap<String, Arc<Mutex<Session>>>>,
    addresses: Arc<PerAddress>,
}

impl Default for Sessions {
    fn default() -> Self {
        Self {
            open: std::sync::Mutex::default(),
            addresses: PerAddress::new(MAX_HTTP_SESSIONS_PER_ADDRESS),
        }
    }
}

struct Session {
    client: Option<Client>,
    queued: Arc<Outbound>,
    outbound: mpsc::UnboundedReceiver<Vec<u8>>,
    /// Frames of earlier posts not yet carried out, because their answers would have queued more
    /// than `OUTBOUND_BYTES`.
    waiting: VecDeque<Vec<u8>>,
    waiting_bytes: usize,
    heard: Instant,
    _place: Held,
}

impl Session {
    /// Whether the session should end: unheard from for too long, or holding more than it may
    /// for a client that does not collect it.
    fn lapsed(&self) -> bool {
        self.heard.elapsed() > IDLE_TIMEOUT || self.queued.overflowed()
    }
}

impl Sessions {
    /// A place for one more session from `address`, or none while the server or the address
    /// holds as many as it may.
    fn reserve(&self, address: IpAddr) -> Option<Held> {
        if self.open.lock().unwrap().len() >= MAX_HTTP_SESSIONS {
            return None;
        }
        self.addresses.take(address)
    }

    fn insert(
        &self,
        client: Client,
        queued: Arc<Outbound>,
        outbound: mpsc::UnboundedReceiver<Vec<u8>>,
        place: Held,
    ) -> Result<String, Client> {
        let mut sessions = self.open.lock().unwrap();
        if sessions.len() >= MAX_HTTP_SESSIONS {
            return Err(client);
        }
        let id = crate::new_secret();
        let session = Session {
            client: Some(client),
            queued,
            outbound,
            waiting: VecDeque::new(),
            waiting_bytes: 0,
            heard: Instant::now(),
            _place: place,
        };
        sessions.insert(id.clone(), Arc::new(Mutex::new(session)));
        Ok(id)
    }

    fn get(&self, id: &str) -> Option<Arc<Mutex<Session>>> {
        self.open.lock().unwrap().get(id).map(Arc::clone)
    }

    pub(crate) async fn sweep(&self) {
        let open: Vec<(String, Arc<Mutex<Session>>)> = self
            .open
            .lock()
            .unwrap()
            .iter()
            .map(|(id, session)| (id.clone(), Arc::clone(session)))
            .collect();
        for (id, session) in open {
            let Ok(mut held) = session.try_lock() else {
                continue;
            };
            if held.client.is_none() || held.lapsed() {
                self.open.lock().unwrap().remove(&id);
                if let Some(client) = held.client.take() {
                    client.close().await;
                }
            }
        }
    }
    pub(crate) async fn revoke_invalid(&self) {
        let sessions: Vec<_> = self.open.lock().unwrap().values().map(Arc::clone).collect();
        for session in sessions {
            let mut session = session.lock().await;
            let Some(client) = &mut session.client else {
                continue;
            };
            // As a websocket does on a session change: a revoked token, a block's agent signed
            // in elsewhere, or its block in the trash.
            if client.connection.signed_in().await.is_err()
                && let Some(client) = session.client.take()
            {
                client.close().await;
            }
        }
    }

    pub(crate) async fn close_all(&self) {
        let open: Vec<Arc<Mutex<Session>>> = self
            .open
            .lock()
            .unwrap()
            .drain()
            .map(|(_, session)| session)
            .collect();
        for session in open {
            if let Some(client) = session.lock().await.client.take() {
                client.close().await;
            }
        }
    }
}

struct Answer {
    status: u16,
    session: Option<String>,
    body: Vec<u8>,
    keep: bool,
    more: bool,
    /// Whether the post's session is signed in.
    signed_in: bool,
}

impl Answer {
    fn refused(status: u16, message: &str, keep: bool) -> Self {
        Self {
            status,
            session: None,
            body: message.as_bytes().to_vec(),
            keep,
            more: false,
            signed_in: false,
        }
    }
}

async fn answer(
    session: Option<&str>,
    authorization: Option<&str>,
    address: IpAddr,
    body: Bytes,
    services: &Services,
) -> Result<Answer, ServerError> {
    let Some(frames) = unframed(&body) else {
        return Ok(Answer::refused(
            400,
            "the body is not a list of frames",
            false,
        ));
    };
    let Some(id) = session else {
        return open_session(frames.first().copied(), authorization, address, services).await;
    };
    let gone = Answer::refused(404, "no such session", true);
    let Some(session) = services.http.get(id) else {
        return Ok(gone);
    };
    let mut held = session.lock().await;
    if held.lapsed() {
        if let Some(client) = held.client.take() {
            client.close().await;
        }
        return Ok(gone);
    }
    held.heard = Instant::now();
    let Session {
        client,
        queued,
        outbound,
        waiting,
        waiting_bytes,
        ..
    } = &mut *held;
    let Some(client) = client.as_mut() else {
        return Ok(gone);
    };
    if client.connection.header_token != crate::bearer(authorization) {
        return Ok(Answer::refused(401, "account authorization changed", false));
    }
    if client.connection.signed_in().await.is_err() {
        if let Some(client) = held.client.take() {
            client.close().await;
        }
        return Ok(gone);
    }
    *waiting_bytes += body.len();
    if *waiting_bytes > MAX_BODY_BYTES {
        if let Some(client) = held.client.take() {
            client.close().await;
        }
        return Ok(Answer::refused(
            413,
            "too many frames wait to be carried out",
            false,
        ));
    }
    waiting.extend(frames.into_iter().map(<[u8]>::to_vec));
    let mut carried = catch_up(client).await;
    // Like the websocket's loop, take no frame while the answers already queued fill a reply;
    // the rest wait for the next post, which the client sends at once.
    while queued.bytes() < OUTBOUND_BYTES
        && carried.is_ok()
        && let Some(frame) = waiting.pop_front()
    {
        *waiting_bytes -= frame.len().min(*waiting_bytes);
        carried = client.receive(&frame).await;
    }
    if waiting.is_empty() {
        *waiting_bytes = 0;
    }
    let carried = carried.and(catch_up(client).await);
    let overflowed = queued.overflowed();
    if let Err(error) = carried.and_then(|()| match overflowed {
        true => Err(crate::not_reading()),
        false => Ok(()),
    }) {
        if let Some(client) = held.client.take() {
            client.close().await;
        }
        return Ok(Answer::refused(400, &error.told(), false));
    }
    let signed_in = client.connection.account.is_some();
    let closing = client.connection.closing;
    let mut answers = Vec::new();
    let mut size = 0;
    while size < OUTBOUND_BYTES
        && let Ok(frame) = outbound.try_recv()
    {
        size += frame.len();
        queued.sent(frame.len());
        answers.push(frame);
    }
    if closing {
        // The session logged out: answer what it said, then end it.
        if let Some(client) = held.client.take() {
            client.close().await;
        }
        return Ok(Answer {
            status: 200,
            session: None,
            body: framed(&answers),
            keep: false,
            more: false,
            signed_in: false,
        });
    }
    Ok(Answer {
        status: 200,
        session: None,
        body: framed(&answers),
        keep: true,
        more: !waiting.is_empty() || !outbound.is_empty(),
        signed_in,
    })
}

/// Answers a post with no session, whose first frame is a greeting: a session it signs in,
/// with the server's greeting saying who as, or the greeting alone when it is of another build
/// or signs nothing in.
async fn open_session(
    greeting_frame: Option<&[u8]>,
    authorization: Option<&str>,
    address: IpAddr,
    services: &Services,
) -> Result<Answer, ServerError> {
    let (ours, greeting) = greeting(greeting_frame.and_then(read_hello).as_ref())?;
    let greeted = |welcome: &Welcome, session: Option<String>| Answer {
        status: 200,
        keep: session.is_some(),
        signed_in: session.is_some(),
        session,
        body: framed(&[hello(ours, welcome)]),
        more: false,
    };
    let Some(greeting) = greeting else {
        return Ok(Answer {
            status: 200,
            session: None,
            body: framed(&[hello(ours, &())]),
            keep: false,
            more: false,
            signed_in: false,
        });
    };
    let Some(place) = services.http.reserve(address) else {
        return Ok(Answer::refused(
            503,
            "too many sessions are open; try again later",
            false,
        ));
    };
    let (sending, outbound) = mpsc::unbounded_channel();
    let queued = Arc::new(Outbound::default());
    let mut client = {
        let queued = Arc::clone(&queued);
        Client::open(services, address, move |frame| {
            if queued.queue(frame.len()) {
                let _ = sending.send(frame);
            }
        })
        .await
    };
    client.connection.header_token = crate::bearer(authorization);
    let welcome = client.greeted(greeting.token).await;
    if welcome.is_err() {
        client.close().await;
        return Ok(greeted(&welcome, None));
    }
    match services.http.insert(client, queued, outbound, place) {
        Ok(id) => Ok(greeted(&welcome, Some(id))),
        Err(client) => {
            client.close().await;
            Ok(Answer::refused(
                503,
                "too many sessions are open; try again later",
                false,
            ))
        }
    }
}

impl IntoResponse for Answer {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (
            status,
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            self.body,
        )
            .into_response();
        let headers = response.headers_mut();
        if let Some(session) = self.session.and_then(|session| session.parse().ok()) {
            headers.insert(SESSION_HEADER, session);
        }
        if self.more {
            headers.insert(MORE_HEADER, HeaderValue::from_static("1"));
        }
        match self.keep {
            true => response,
            false => closing(response),
        }
    }
}

/// Answers one call that needs no sign-in (`device-login`, and `query.invitation`) posted after
/// a greeting, then closes: signing in through the browser holds no connection while the person
/// approves it, and an invitation is read before its holder has an account.
async fn call(
    State(services): State<Services>,
    Extension(Admission { client, link }): Extension<Admission>,
    body: Body,
) -> Response {
    // Held until the call is answered: a one-shot call is never signed in.
    let anonymous = link.places.lock().unwrap().anonymous.take();
    let answered = one_shot(body, client, &services).await;
    drop(anonymous);
    closing(answered.unwrap_or_else(failed))
}

async fn one_shot(
    body: Body,
    address: IpAddr,
    services: &Services,
) -> Result<Response, ServerError> {
    if !services.calls.allow(address) {
        return Ok(refuse(
            StatusCode::TOO_MANY_REQUESTS,
            "too many calls from this address; wait a minute",
        ));
    }
    let body = match read(body, Some(HANDSHAKE_TIMEOUT)).await {
        Ok(body) => body,
        Err(refused) => return Ok(refused),
    };
    let refused = || {
        Ok(refuse(
            StatusCode::BAD_REQUEST,
            "post a greeting and one call",
        ))
    };
    let Some([greeting_frame, call]) =
        unframed(&body).and_then(|frames| <[&[u8]; 2]>::try_from(frames).ok())
    else {
        return refused();
    };
    let (ours, greeting) = greeting(read_hello(greeting_frame).as_ref())?;
    let ours = hello(ours, &());
    if greeting.is_none() {
        return Ok(framed_answer(&[ours]));
    }
    let method = match be_wasm_ops::decode_frame::<be_wasm_ops::Frame>(call) {
        Ok(be_wasm_ops::Frame::Start { method, .. }) => method,
        _ => return refused(),
    };
    // Signing in through the browser, and reading an invitation (`query.invitation`), which
    // `client::query_rule` lets anyone make: any other read is refused as not signed in.
    let allowed = method == be_block_protocol::__args::SERVER_DEVICE_LOGIN
        || method == be_block_protocol::__args::SERVER_READ;
    if !allowed {
        return Ok(refuse(
            StatusCode::FORBIDDEN,
            "only signing in and reading an invitation are one-shot calls; connect with a token for the rest",
        ));
    }
    let (sending, mut answered) = mpsc::unbounded_channel();
    let mut client = Client::open(services, address, move |frame| {
        let _ = sending.send(frame);
    })
    .await;
    let carried = client.receive(call).await;
    client.close().await;
    carried?;
    let mut frames = vec![ours];
    while let Ok(frame) = answered.try_recv() {
        frames.push(frame);
    }
    Ok(framed_answer(&frames))
}

fn framed_answer(frames: &[Vec<u8>]) -> Response {
    Answer {
        status: 200,
        session: None,
        body: framed(frames),
        keep: false,
        more: false,
        signed_in: false,
    }
    .into_response()
}

async fn catch_up(client: &mut Client) -> Result<(), ServerError> {
    for late in client.pending_lates() {
        client.late(late).await;
    }
    for push in client.pending_pushes() {
        client.push(push).await?;
    }
    Ok(())
}

fn unframed(body: &[u8]) -> Option<Vec<&[u8]>> {
    let mut frames = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let size = u32::from_be_bytes(rest.get(..4)?.try_into().ok()?) as usize;
        frames.push(rest.get(4..4 + size)?);
        rest = &rest[4 + size..];
    }
    Some(frames)
}

fn framed(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for frame in frames {
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend_from_slice(frame);
    }
    body
}
