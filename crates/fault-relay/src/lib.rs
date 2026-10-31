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
    links: Mutex<Vec<Link>>,
    holding: Mutex<bool>,
    stalling: Mutex<bool>,
    jamming: Mutex<bool>,
    heard: watch::Sender<u64>,
    counters: Mutex<Vec<Counter>>,
}

type Counter = (
    Box<dyn Fn(be_ops::MethodId) -> bool + Send>,
    watch::Sender<u64>,
);

#[derive(Clone)]
struct Link {
    stalled: watch::Sender<bool>,
    jammed: watch::Sender<bool>,
    cut: watch::Sender<bool>,
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

    pub fn heard(&self) -> u64 {
        *self.state.heard.borrow()
    }

    pub async fn heard_beyond(&self, bytes: u64) {
        let mut heard = self.state.heard.subscribe();
        let _ = heard.wait_for(|heard| *heard > bytes).await;
    }

    pub fn count(
        &self,
        counted: impl Fn(be_ops::MethodId) -> bool + Send + 'static,
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
        };
        state
            .links
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(link.clone());
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
    loop {
        if *link.jammed.borrow() {
            std::future::pending::<()>().await;
        }
        let read = match from.read(&mut buffer).await {
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
    fn messages_in(&mut self, bytes: &[u8]) -> Vec<be_ops::MethodId> {
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
            if let Ok(be_ops::Frame::Start { method, .. }) = be_ops::decode_frame(&payload) {
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
