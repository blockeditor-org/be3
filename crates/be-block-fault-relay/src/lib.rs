use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpListener, TcpSocket, TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::watch,
};

pub struct Relay {
    address: SocketAddr,
    state: Arc<State>,
    runtime: Option<tokio::runtime::Runtime>,
}

struct State {
    upstream: SocketAddr,
    offline: Mutex<bool>,
    links: Mutex<Vec<Link>>,
    holding: Mutex<bool>,
    stalling: Mutex<bool>,
    jamming: Mutex<bool>,
    heard: watch::Sender<u64>,
    counters: Mutex<Vec<Counter>>,
}

type Counter = (
    Box<dyn Fn(be_wasm_ops::MethodId) -> bool + Send>,
    watch::Sender<u64>,
);

#[derive(Clone)]
struct Link {
    stalled: watch::Sender<bool>,
    jammed: watch::Sender<bool>,
    cut: watch::Sender<bool>,
    /// Requests (to the server) and answers (to the client) held back, delivered in order once
    /// let go (`hold_requests`, `hold_answers`).
    holding_requests: watch::Sender<bool>,
    holding_answers: watch::Sender<bool>,
}

impl Relay {
    pub fn start(upstream: SocketAddr) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("fault-relay")
            .enable_io()
            .build()
            .expect("the relay's runtime starts");
        let listener = {
            let _entered = runtime.enter();
            let socket = TcpSocket::new_v4().expect("the relay opens a socket");
            socket
                .set_recv_buffer_size(RECEIVE_BUFFER)
                .expect("the relay sizes what it buffers");
            socket
                .bind("127.0.0.1:0".parse().expect("a local address parses"))
                .expect("the relay binds");
            socket.listen(64).expect("the relay listens")
        };
        let address = listener.local_addr().expect("the relay has an address");
        let state = Arc::new(State {
            upstream,
            offline: Mutex::new(false),
            links: Mutex::new(Vec::new()),
            holding: Mutex::new(false),
            stalling: Mutex::new(false),
            jamming: Mutex::new(false),
            heard: watch::Sender::new(0),
            counters: Mutex::new(Vec::new()),
        });
        runtime.spawn(accept(listener, Arc::clone(&state)));
        Self {
            address,
            state,
            runtime: Some(runtime),
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn url(&self) -> String {
        format!("ws://{}", self.address)
    }

    pub fn stall(&self) {
        for link in self.state.links() {
            link.stalled.send_replace(true);
        }
    }

    pub fn stall_at_next_request(&self) {
        *self
            .state
            .stalling
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
    }

    pub fn jam_at_next_request(&self) {
        *self
            .state
            .jamming
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
    }

    /// Holds back what the connections open now send the server, unlike `stall`, which drops it:
    /// delivered in order once let go.
    pub fn hold_requests(&self, holding: bool) {
        for link in self.state.links() {
            link.holding_requests.send_replace(holding);
        }
    }

    /// Holds back what the server answers on the connections open now: a request the server
    /// has answered reaches the client only once let go.
    pub fn hold_answers(&self, holding: bool) {
        for link in self.state.links() {
            link.holding_answers.send_replace(holding);
        }
    }

    pub fn cut(&self) {
        for link in self.state.links() {
            link.cut.send_replace(true);
        }
    }

    pub fn hold_new(&self, holding: bool) {
        *self
            .state
            .holding
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = holding;
    }

    /// Takes the server away, as a network that went down: cuts every connection and closes
    /// each new one as soon as it is accepted, so a client learns at once that it cannot
    /// connect, until `offline(false)`.
    pub fn offline(&self, offline: bool) {
        *self
            .state
            .offline
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = offline;
        if offline {
            self.cut();
        }
    }

    pub fn heard(&self) -> u64 {
        *self.state.heard.borrow()
    }

    pub async fn heard_beyond(&self, bytes: u64) {
        let mut heard = self.state.heard.subscribe();
        let _ = heard.wait_for(|heard| *heard > bytes).await;
    }

    pub fn count(
        &self,
        counted: impl Fn(be_wasm_ops::MethodId) -> bool + Send + 'static,
    ) -> watch::Receiver<u64> {
        let (sender, receiver) = watch::channel(0);
        self.state
            .counters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((Box::new(counted), sender));
        receiver
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.cut();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl State {
    fn links(&self) -> Vec<Link> {
        let mut links = self.links.lock().unwrap_or_else(PoisonError::into_inner);
        links.retain(|link| !*link.cut.borrow());
        links.clone()
    }
}

async fn accept(listener: TcpListener, state: Arc<State>) {
    while let Ok((client, _)) = listener.accept().await {
        let holding = *state.holding.lock().unwrap_or_else(PoisonError::into_inner);
        let link = Link {
            stalled: watch::Sender::new(holding),
            jammed: watch::Sender::new(false),
            cut: watch::Sender::new(false),
            holding_requests: watch::Sender::new(false),
            holding_answers: watch::Sender::new(false),
        };
        {
            // Checked under the links' lock, which `offline` takes to cut them after setting
            // the flag: a connection is either refused here or listed in time to be cut.
            let mut links = state.links.lock().unwrap_or_else(PoisonError::into_inner);
            if *state.offline.lock().unwrap_or_else(PoisonError::into_inner) {
                drop(client);
                continue;
            }
            links.push(link.clone());
        }
        tokio::spawn(carry(client, link, Arc::clone(&state), holding));
    }
}

async fn carry(client: TcpStream, link: Link, state: Arc<State>, holding: bool) {
    let mut cut = link.cut.subscribe();
    let (mut from_client, mut to_client) = client.into_split();
    let server = if holding {
        None
    } else {
        TcpStream::connect(state.upstream).await.ok()
    };
    let carried = async {
        let Some(server) = server else {
            pump(&mut from_client, None, &link, Some(&state)).await;
            return;
        };
        let (mut from_server, mut to_server) = server.into_split();
        let mut up = std::pin::pin!(pump(
            &mut from_client,
            Some(&mut to_server),
            &link,
            Some(&state)
        ));
        let down = std::pin::pin!(pump(&mut from_server, Some(&mut to_client), &link, None));
        tokio::select! {
            () = &mut up => {}
            () = down => {
                if *link.stalled.borrow() {
                    up.await;
                }
            }
        }
    };
    tokio::select! {
        () = carried => {}
        _ = cut.wait_for(|cut| *cut) => {}
    }
    link.cut.send_replace(true);
}

async fn pump(
    from: &mut OwnedReadHalf,
    mut to: Option<&mut OwnedWriteHalf>,
    link: &Link,
    heard: Option<&State>,
) {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut frames = Frames::default();
    let holding = match heard {
        Some(_) => &link.holding_requests,
        None => &link.holding_answers,
    };
    let mut let_go = holding.subscribe();
    let mut held: Vec<u8> = Vec::new();
    loop {
        if *link.jammed.borrow() {
            std::future::pending::<()>().await;
        }
        let read = tokio::select! {
            read = from.read(&mut buffer) => Some(read),
            let_go = let_go.wait_for(|holding| !*holding), if !held.is_empty() => {
                drop(let_go);
                None
            }
        };
        let Some(read) = read else {
            // Let go: what was held goes now, in order.
            if let Some(to) = &mut to
                && to.write_all(&held).await.is_err()
            {
                return;
            }
            held.clear();
            continue;
        };
        let read = match read {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        if let Some(state) = heard {
            if std::mem::take(
                &mut *state
                    .stalling
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner),
            ) {
                for link in state.links() {
                    link.stalled.send_replace(true);
                }
            }
            if std::mem::take(&mut *state.jamming.lock().unwrap_or_else(PoisonError::into_inner)) {
                for link in state.links() {
                    link.jammed.send_replace(true);
                }
            }
            state.heard.send_modify(|heard| *heard += read as u64);
            let messages = frames.messages_in(&buffer[..read]);
            if !messages.is_empty() {
                let counters = state
                    .counters
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                for (counted, count) in counters.iter() {
                    let matched = messages.iter().filter(|method| counted(**method)).count();
                    if matched > 0 {
                        count.send_modify(|count| *count += matched as u64);
                    }
                }
            }
        }
        if *link.stalled.borrow() {
            continue;
        }
        if *holding.borrow() {
            held.extend_from_slice(&buffer[..read]);
            continue;
        }
        if let Some(to) = &mut to
            && !held.is_empty()
        {
            if to.write_all(&held).await.is_err() {
                return;
            }
            held.clear();
        }
        if let Some(to) = &mut to
            && to.write_all(&buffer[..read]).await.is_err()
        {
            return;
        }
    }
}

const RECEIVE_BUFFER: u32 = 64 * 1024;

#[derive(Default)]
struct Frames {
    upgraded: bool,
    unread: Vec<u8>,
}

impl Frames {
    fn messages_in(&mut self, bytes: &[u8]) -> Vec<be_wasm_ops::MethodId> {
        self.unread.extend_from_slice(bytes);
        if !self.upgraded {
            let Some(end) = self
                .unread
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
            else {
                return Vec::new();
            };
            self.unread.drain(..end + 4);
            self.upgraded = true;
        }
        let mut messages = Vec::new();
        while let Some((payload, length)) = frame(&self.unread) {
            if let Ok(be_wasm_ops::Frame::Start { method, .. }) =
                be_wasm_ops::decode_frame(&payload)
            {
                messages.push(method);
            }
            self.unread.drain(..length);
        }
        messages
    }
}

fn frame(bytes: &[u8]) -> Option<(Vec<u8>, usize)> {
    let [first, second, ..] = *bytes else {
        return None;
    };
    let (length, mut at) = match second & 0x7f {
        126 => (
            u64::from(u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?)),
            4,
        ),
        127 => (u64::from_be_bytes(bytes.get(2..10)?.try_into().ok()?), 10),
        length => (u64::from(length), 2),
    };
    let mask: [u8; 4] = if second & 0x80 == 0 {
        [0; 4]
    } else {
        at += 4;
        bytes.get(at - 4..at)?.try_into().ok()?
    };
    let end = at + usize::try_from(length).ok()?;
    let payload = bytes.get(at..end)?;
    let binary = first & 0x0f == 2;
    let payload = if binary {
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4])
            .collect()
    } else {
        Vec::new()
    };
    Some((payload, end))
}
