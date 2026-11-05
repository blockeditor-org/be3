use std::{
    collections::{HashMap, VecDeque},
    io,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use be_block_protocol::{MAX_FRAME_BYTES, read_hello};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    sync::{Mutex, mpsc},
};
use uuid::Uuid;

use crate::{
    Admitted, Client, HANDSHAKE_TIMEOUT, IDLE_TIMEOUT, OUTBOUND_BYTES, Outbound, ServerError,
    Services,
    address::{Anonymous, Eviction, Held, PerAddress, Unsigned},
    greeting,
};

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
const SESSION_HEADER: &str = "be3-session";
/// Set on an answer when the session has more to say, or frames of the post still to carry
/// out; the client posts again at once, with an empty body if it has nothing to send.
const MORE_HEADER: &str = "be3-more";
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 2 * MAX_FRAME_BYTES;
const MAX_FORM_BYTES: usize = 16 * 1024;
/// Longest a request body may go without a byte arriving.
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Bytes read from a socket at a time, so a body's buffer grows only as its bytes arrive.
const READ_CHUNK_BYTES: usize = 64 * 1024;
const MAX_HEADERS: usize = 64;
/// HTTP sessions open at once on the whole server, and for one client address.
const MAX_HTTP_SESSIONS: usize = 1024;
pub(crate) const MAX_HTTP_SESSIONS_PER_ADDRESS: usize = 32;
/// HTTP sessions not signed in yet, on the whole server; opening one past this lets go of the
/// oldest, so they never hold more than this of `MAX_HTTP_SESSIONS`.
pub(crate) const MAX_UNSIGNED_HTTP_SESSIONS: usize = MAX_HTTP_SESSIONS / 4;
/// HTTP sessions not signed in that one address (an IPv6 /48) may hold.
pub(crate) const MAX_UNSIGNED_HTTP_SESSIONS_PER_ADDRESS: usize = 4;

pub(crate) struct Sessions {
    open: std::sync::Mutex<HashMap<String, Open>>,
    addresses: Arc<PerAddress>,
    pub(crate) unsigned: Arc<Unsigned>,
}

#[derive(Clone)]
struct Open {
    session: Arc<Mutex<Session>>,
    eviction: Arc<Eviction>,
}

impl Default for Sessions {
    fn default() -> Self {
        Self {
            open: std::sync::Mutex::default(),
            addresses: PerAddress::new(MAX_HTTP_SESSIONS_PER_ADDRESS),
            unsigned: Unsigned::new(
                MAX_UNSIGNED_HTTP_SESSIONS,
                MAX_UNSIGNED_HTTP_SESSIONS_PER_ADDRESS,
            ),
        }
    }
}

/// What a new HTTP session holds: its place among its address's sessions, and among those not
/// signed in.
pub(crate) struct Reserved {
    place: Held,
    anonymous: Anonymous,
    eviction: Arc<Eviction>,
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
    /// Whether the session should end: unheard from for too long, out of time to sign in (or
    /// let go of for a newer session that is not signed in), or holding more than it may for a
    /// client that does not collect it.
    fn lapsed(&self) -> bool {
        self.heard.elapsed() > IDLE_TIMEOUT
            || self.queued.overflowed()
            || self.client.as_ref().is_some_and(|client| {
                client
                    .connection
                    .sign_in_deadline()
                    .is_some_and(|deadline| deadline <= tokio::time::Instant::now())
            })
    }
}

impl Sessions {
    /// A place for one more session from `address`, or none while the server or the address
    /// holds as many as it may.
    fn reserve(&self, address: IpAddr) -> Option<Reserved> {
        if self.open.lock().unwrap().len() >= MAX_HTTP_SESSIONS && !self.drop_evicted() {
            return None;
        }
        let place = self.addresses.take(address)?;
        let eviction = Arc::new(Eviction::default());
        let anonymous = self.unsigned.admit(address, &eviction)?;
        Some(Reserved {
            place,
            anonymous,
            eviction,
        })
    }

    /// Ends the sessions let go of for newer ones not signed in, which the next sweep would;
    /// answers whether that made room.
    fn drop_evicted(&self) -> bool {
        let evicted: Vec<Open> = {
            let mut open = self.open.lock().unwrap();
            let ids: Vec<String> = open
                .iter()
                .filter(|(_, open)| open.eviction.evicted())
                .map(|(id, _)| id.clone())
                .collect();
            ids.iter().filter_map(|id| open.remove(id)).collect()
        };
        let made = !evicted.is_empty();
        for open in evicted {
            tokio::spawn(async move {
                if let Some(client) = open.session.lock().await.client.take() {
                    client.close().await;
                }
            });
        }
        made
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
        let eviction = Arc::clone(&client.connection.eviction);
        let id = Uuid::new_v4().simple().to_string();
        let session = Session {
            client: Some(client),
            queued,
            outbound,
            waiting: VecDeque::new(),
            waiting_bytes: 0,
            heard: Instant::now(),
            _place: place,
        };
        sessions.insert(
            id.clone(),
            Open {
                session: Arc::new(Mutex::new(session)),
                eviction,
            },
        );
        Ok(id)
    }

    fn get(&self, id: &str) -> Option<Arc<Mutex<Session>>> {
        self.open
            .lock()
            .unwrap()
            .get(id)
            .map(|open| Arc::clone(&open.session))
    }

    pub(crate) async fn sweep(&self) {
        let open: Vec<(String, Arc<Mutex<Session>>)> = self
            .open
            .lock()
            .unwrap()
            .iter()
            .map(|(id, open)| (id.clone(), Arc::clone(&open.session)))
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
    pub(crate) async fn revoke_invalid(&self, store: &crate::ServerStore) {
        let sessions: Vec<_> = self
            .open
            .lock()
            .unwrap()
            .values()
            .map(|open| Arc::clone(&open.session))
            .collect();
        for session in sessions {
            let mut session = session.lock().await;
            let Some(client) = &session.client else {
                continue;
            };
            if let Some(token) = &client.connection.token
                && store.resolve_token(token).await.is_err()
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
            .map(|(_, open)| open.session)
            .collect();
        for session in open {
            if let Some(client) = session.lock().await.client.take() {
                client.close().await;
            }
        }
    }
}

/// A request that asks for a websocket, with what the socket needs from its head.
pub(crate) struct Upgrade {
    pub(crate) stream: Prefixed,
    pub(crate) client: IpAddr,
    pub(crate) authorization: Option<String>,
    /// The connection's place among its address's, held while it is open.
    pub(crate) place: Held,
    /// Its place among the connections not signed in, held until it signs in.
    pub(crate) anonymous: Anonymous,
}

/// Reads a request's head and serves it, or hands back a websocket upgrade. `place` is the
/// connection's place among its peer's; a trusted proxy's connection has none yet and takes
/// one for the address it forwards.
pub(crate) async fn route(
    mut stream: TcpStream,
    peer: SocketAddr,
    admitted: Admitted,
    services: &Services,
) -> Result<Option<Upgrade>, ServerError> {
    let Admitted {
        place,
        anonymous,
        eviction,
    } = admitted;
    let mut buffer = Vec::new();
    let read = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_head(&mut stream, &mut buffer))
        .await
        .map_err(|_| ServerError::Socket("the request never finished its head".into()))??;
    if !read {
        return Ok(None);
    }
    let head = Head::parse(&buffer)?;
    let client = services.client_address(peer.ip(), head.forwarded.as_deref());
    let (Some(place), Some(anonymous)) = (
        place.or_else(|| services.addresses.take(client)),
        anonymous.or_else(|| services.unsigned.admit(client, &eviction)),
    ) else {
        tracing::debug!(%client, "refusing a request: its address holds too many connections");
        refuse(&mut stream, 429, "too many connections from this address").await?;
        return Ok(None);
    };
    if head
        .path
        .split('?')
        .next()
        .is_some_and(|path| path == "/auth" || path.starts_with("/auth/"))
    {
        serve_auth(stream, buffer, client, services).await?;
        return Ok(None);
    }
    if services.config.local_profiles && head.origin.is_some() {
        refuse(
            &mut stream,
            403,
            "browser access to local profiles is unavailable",
        )
        .await?;
        return Ok(None);
    }
    let path = head.path.split('?').next().unwrap_or_default();
    if s3::is_s3(path) {
        s3::serve(stream, buffer, &services.s3).await?;
        return Ok(None);
    }
    if head.method == "POST" && path.ends_with(CALL_SUFFIX) {
        one_shot(stream, buffer, client, (anonymous, eviction), services).await?;
        return Ok(None);
    }
    if head.method == "POST" && path.ends_with(PATH_SUFFIX) {
        serve(stream, peer, buffer, anonymous, services).await?;
        return Ok(None);
    }
    Ok(Some(Upgrade {
        stream: Prefixed {
            prefix: buffer,
            position: 0,
            stream,
        },
        client,
        authorization: head.authorization,
        place,
        anonymous,
    }))
}

async fn serve_auth(
    mut stream: TcpStream,
    mut buffer: Vec<u8>,
    client: IpAddr,
    services: &Services,
) -> Result<(), ServerError> {
    let head = Head::parse(&buffer)?;
    if head.length > MAX_FORM_BYTES {
        return refuse(&mut stream, 413, "the form is too large").await;
    }
    let host = head.host.as_deref().unwrap_or_default();
    let local = host == "localhost"
        || host.starts_with("localhost:")
        || host == "127.0.0.1"
        || host.starts_with("127.0.0.1:")
        || host == "[::1]"
        || host.starts_with("[::1]:");
    let origin = services
        .config
        .auth_origin
        .clone()
        .unwrap_or_else(|| format!("{}://{host}", if local { "http" } else { "https" }));
    if !local && services.config.auth_origin.is_none() {
        return refuse(
            &mut stream,
            503,
            "The server operator must configure --auth-origin before using the account portal.",
        )
        .await;
    }
    let (path, query) = head.path.split_once('?').unwrap_or((&head.path, ""));
    if head.method == "GET"
        && let Some(asset) = services
            .portal
            .as_ref()
            .and_then(|served| served.asset(path))
    {
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nCache-Control: public, max-age=31536000, immutable\r\nCross-Origin-Resource-Policy: same-origin\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            asset.content_type,
            asset.bytes.len()
        );
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(asset.bytes).await?;
        stream.shutdown().await?;
        return Ok(());
    }
    let query = crate::browser_auth::fields(query);
    let mut fields = HashMap::new();
    if head.method == "POST" {
        if head.origin.as_deref() != Some(origin.as_str()) {
            return refuse(
                &mut stream,
                403,
                "submit this form from the server's account portal",
            )
            .await;
        }
        if head
            .content_type
            .as_deref()
            .and_then(|value| value.split(';').next())
            != Some("application/x-www-form-urlencoded")
        {
            return refuse(&mut stream, 415, "use the account portal's form").await;
        }
        let mut body = buffer.split_off(head.size);
        tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            read_body(&mut stream, &mut body, head.length),
        )
        .await
        .map_err(|_| ServerError::Socket("the form timed out".into()))??
        .then_some(())
        .ok_or_else(|| ServerError::Socket("incomplete form".into()))?;
        fields = crate::browser_auth::fields(
            std::str::from_utf8(&body[..head.length]).unwrap_or_default(),
        );
    }
    let token = head
        .cookie
        .as_deref()
        .unwrap_or_default()
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find(|(key, _)| *key == "be3-account")
        .map(|(_, value)| value)
        .unwrap_or_default();
    let result = crate::portal::handle(
        services,
        &head.method,
        path,
        &query,
        &fields,
        token,
        &origin,
        client,
    )
    .await?;
    let mut extras = String::new();
    if let Some(location) = result.location {
        extras.push_str(&format!("Location: {location}\r\n"));
    }
    if let Some(cookie) = result.cookie {
        extras.push_str(&format!("Set-Cookie: be3-account={cookie}; Path=/auth; HttpOnly; SameSite=Lax; Max-Age={}{}\r\n", if cookie.is_empty() { 0 } else { 30 * 86400 }, if origin.starts_with("https://") { "; Secure" } else { "" }));
    }
    let (content_type, body, page_headers) = match result.body {
        crate::portal::Body::Html(html) => (
            "text/html; charset=utf-8",
            html,
            format!(
                "Content-Security-Policy: {}\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n",
                services
                    .portal
                    .as_ref()
                    .map(|served| served.policy.as_str())
                    .unwrap_or_default()
            ),
        ),
        crate::portal::Body::Json(json) => (
            "application/json",
            json,
            "Content-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\n".to_owned(),
        ),
    };
    let status = result.status;
    let response = format!(
        "HTTP/1.1 {status} Response\r\n{extras}Content-Type: {content_type}\r\nCache-Control: no-store\r\n{page_headers}X-Content-Type-Options: nosniff\r\nReferrer-Policy: strict-origin\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

/// Serves the posts of one connection. It counts as not signed in, and can be let go of for a
/// newer one, until a post it carries is for a session that is signed in.
async fn serve(
    mut stream: TcpStream,
    peer: SocketAddr,
    mut buffer: Vec<u8>,
    anonymous: Anonymous,
    services: &Services,
) -> Result<(), ServerError> {
    let mut anonymous = Some(anonymous);
    loop {
        let head = Head::parse(&buffer)?;
        if head.length > MAX_BODY_BYTES {
            return refuse(&mut stream, 413, "the frame is too large").await;
        }
        let mut body = buffer.split_off(head.size);
        if !read_body(&mut stream, &mut body, head.length).await? {
            return Ok(());
        }
        buffer = body.split_off(head.length);
        // A proxy's connection carries many clients' requests, so each request is counted by
        // the address its own head forwards.
        let client = services.client_address(peer.ip(), head.forwarded.as_deref());
        let answer = answer(
            head.session.as_deref(),
            head.authorization.as_deref(),
            client,
            body,
            services,
        )
        .await?;
        if answer.signed_in {
            drop(anonymous.take());
        }
        respond(
            &mut stream,
            answer.status,
            answer.session.as_deref(),
            answer.more,
            "application/octet-stream",
            &answer.body,
        )
        .await?;
        if head.close || !answer.keep {
            return Ok(());
        }
        match tokio::time::timeout(IDLE_TIMEOUT, read_head(&mut stream, &mut buffer)).await {
            Ok(Ok(true)) => {}
            Ok(Err(error)) => return Err(error),
            Ok(Ok(false)) | Err(_) => return Ok(()),
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
    body: Vec<u8>,
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
        let theirs = frames.first().and_then(|frame| read_hello(frame));
        let (ours, compatible) = greeting(theirs.as_deref())?;
        let session = match compatible {
            true => {
                let Some(Reserved {
                    place,
                    anonymous,
                    eviction,
                }) = services.http.reserve(address)
                else {
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
                    Client::open(
                        services,
                        address,
                        false,
                        (anonymous, eviction),
                        move |frame| {
                            if queued.queue(frame.len()) {
                                let _ = sending.send(frame);
                            }
                        },
                    )
                    .await
                };
                client.connection.header_token = crate::bearer(authorization);
                match services.http.insert(client, queued, outbound, place) {
                    Ok(id) => Some(id),
                    Err(client) => {
                        client.close().await;
                        return Ok(Answer::refused(
                            503,
                            "too many sessions are open; try again later",
                            false,
                        ));
                    }
                }
            }
            false => None,
        };
        return Ok(Answer {
            status: 200,
            session,
            body: framed(&[ours]),
            keep: compatible,
            more: false,
            signed_in: false,
        });
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
        return Ok(Answer::refused(400, &error.to_string(), false));
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
        // The session's first call did not sign it in, or it logged out: answer what it said,
        // then end it.
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

/// Answers one call that needs no sign-in (`begin-browser-login`, `poll-browser-login`, and
/// `create-local-profile` where local profiles are on) posted after a greeting, then closes:
/// signing in through the browser holds no connection while the person approves it.
async fn one_shot(
    mut stream: TcpStream,
    mut buffer: Vec<u8>,
    address: IpAddr,
    anonymous: (Anonymous, Arc<Eviction>),
    services: &Services,
) -> Result<(), ServerError> {
    let head = Head::parse(&buffer)?;
    if head.length > MAX_CALL_BYTES {
        return refuse(&mut stream, 413, "a one-shot call is short").await;
    }
    if !services.calls.allow(address) {
        return refuse(
            &mut stream,
            429,
            "too many calls from this address; wait a minute",
        )
        .await;
    }
    let mut body = buffer.split_off(head.size);
    let read = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        read_body(&mut stream, &mut body, head.length),
    )
    .await
    .map_err(|_| ServerError::Socket("the call timed out".into()))??;
    if !read {
        return Ok(());
    }
    let Some([greeting_frame, call]) =
        unframed(&body[..head.length]).and_then(|frames| <[&[u8]; 2]>::try_from(frames).ok())
    else {
        return refuse(&mut stream, 400, "post a greeting and one call").await;
    };
    let (ours, compatible) = greeting(read_hello(greeting_frame).as_deref())?;
    if !compatible {
        return respond_once(&mut stream, framed(&[ours])).await;
    }
    let method = match be_wasm_ops::decode_frame::<be_wasm_ops::Frame>(call) {
        Ok(be_wasm_ops::Frame::Start { method, .. }) => method,
        _ => return refuse(&mut stream, 400, "post a greeting and one call").await,
    };
    let allowed = method == be_block_protocol::__args::ACCOUNT_BEGIN_BROWSER_LOGIN
        || method == be_block_protocol::__args::ACCOUNT_POLL_BROWSER_LOGIN
        || (method == be_block_protocol::__args::ACCOUNT_CREATE_LOCAL_PROFILE
            && services.config.local_profiles);
    if !allowed {
        return refuse(
            &mut stream,
            403,
            "only signing in is a one-shot call; connect and authenticate for the rest",
        )
        .await;
    }
    let (sending, mut answered) = mpsc::unbounded_channel();
    let mut client = Client::open(services, address, true, anonymous, move |frame| {
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
    respond_once(&mut stream, framed(&frames)).await
}

async fn respond_once(stream: &mut TcpStream, body: Vec<u8>) -> Result<(), ServerError> {
    respond(stream, 200, None, false, "application/octet-stream", &body).await?;
    stream.shutdown().await?;
    Ok(())
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

async fn refuse(stream: &mut TcpStream, status: u16, message: &str) -> Result<(), ServerError> {
    respond(
        stream,
        status,
        None,
        false,
        "text/plain; charset=utf-8",
        message.as_bytes(),
    )
    .await
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    session: Option<&str>,
    more: bool,
    content_type: &str,
    body: &[u8],
) -> Result<(), ServerError> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nCache-Control: no-store\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(session) = session {
        head.push_str(&format!("{SESSION_HEADER}: {session}\r\n"));
    }
    if more {
        head.push_str(&format!("{MORE_HEADER}: 1\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

/// Reads until `body` holds `length` bytes, growing it only as bytes arrive and giving up when
/// none arrive for `BODY_READ_TIMEOUT`; false if the peer closed first.
async fn read_body(
    stream: &mut TcpStream,
    body: &mut Vec<u8>,
    length: usize,
) -> Result<bool, ServerError> {
    let mut chunk = vec![0; READ_CHUNK_BYTES.min(length.saturating_sub(body.len()))];
    while body.len() < length {
        let wanted = chunk.len().min(length - body.len());
        let read = tokio::time::timeout(BODY_READ_TIMEOUT, stream.read(&mut chunk[..wanted]))
            .await
            .map_err(|_| ServerError::Socket("the request body stalled".into()))??;
        if read == 0 {
            return Ok(false);
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(true)
}

async fn read_head(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<bool, ServerError> {
    let mut chunk = [0u8; 4096];
    while !buffer.windows(4).any(|window| window == b"\r\n\r\n") {
        if buffer.len() > MAX_HEAD_BYTES {
            return Err(ServerError::Socket("the request head is too long".into()));
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(false);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(true)
}

struct Head {
    method: String,
    path: String,
    size: usize,
    length: usize,
    session: Option<String>,
    close: bool,
    authorization: Option<String>,
    origin: Option<String>,
    host: Option<String>,
    cookie: Option<String>,
    content_type: Option<String>,
    forwarded: Option<String>,
}

impl Head {
    fn parse(buffer: &[u8]) -> Result<Self, ServerError> {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut request = httparse::Request::new(&mut headers);
        let size = match request.parse(buffer) {
            Ok(httparse::Status::Complete(size)) => size,
            _ => return Err(ServerError::Socket("the request head is malformed".into())),
        };
        let header = |name: &str| {
            request
                .headers
                .iter()
                .find(|header| header.name.eq_ignore_ascii_case(name))
                .and_then(|header| std::str::from_utf8(header.value).ok())
                .map(str::trim)
        };
        let length = match header("content-length") {
            Some(value) => value
                .parse()
                .map_err(|_| ServerError::Socket("the content length is not a number".into()))?,
            None => 0,
        };
        let session = header(SESSION_HEADER).map(str::to_owned);
        let close = header("connection").is_some_and(|value| value.eq_ignore_ascii_case("close"));
        Ok(Self {
            method: request.method.unwrap_or_default().to_owned(),
            path: request.path.unwrap_or_default().to_owned(),
            size,
            length,
            session,
            close,
            authorization: header("authorization").map(str::to_owned),
            origin: header("origin").map(str::to_owned),
            host: header("host").map(str::to_owned),
            cookie: header("cookie").map(str::to_owned),
            content_type: header("content-type").map(str::to_owned),
            forwarded: header("x-forwarded-for").map(str::to_owned),
        })
    }
}

pub(crate) struct Prefixed {
    prefix: Vec<u8>,
    position: usize,
    stream: TcpStream,
}

impl AsyncRead for Prefixed {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.position < self.prefix.len() {
            let left = &self.prefix[self.position..];
            let count = left.len().min(buffer.remaining());
            buffer.put_slice(&left[..count]);
            self.position += count;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.stream).poll_read(context, buffer)
    }
}

impl AsyncWrite for Prefixed {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(context, buffer)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}
