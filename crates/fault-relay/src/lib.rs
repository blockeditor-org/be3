use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpListener, TcpStream,
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
    heard: watch::Sender<u64>,
    pings: watch::Sender<u64>,
}

#[derive(Clone)]
struct Link {
    stalled: watch::Sender<bool>,
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
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("the relay listens");
        listener
            .set_nonblocking(true)
            .expect("the relay's socket does not block");
        let address = listener.local_addr().expect("the relay has an address");
        let state = Arc::new(State {
            upstream,
            links: Mutex::new(Vec::new()),
            holding: Mutex::new(false),
            stalling: Mutex::new(false),
            heard: watch::Sender::new(0),
            pings: watch::Sender::new(0),
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

    pub fn pings(&self) -> u64 {
        *self.state.pings.borrow()
    }

    pub async fn pinged_beyond(&self, pings: u64) {
        let mut pinged = self.state.pings.subscribe();
        let _ = pinged.wait_for(|pinged| *pinged > pings).await;
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

async fn accept(listener: std::net::TcpListener, state: Arc<State>) {
    let Ok(listener) = TcpListener::from_std(listener) else {
        return;
    };
    while let Ok((client, _)) = listener.accept().await {
        let holding = *state.holding.lock().unwrap_or_else(PoisonError::into_inner);
        let link = Link {
            stalled: watch::Sender::new(holding),
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
            state.heard.send_modify(|heard| *heard += read as u64);
            let pings = frames.pings_in(&buffer[..read]);
            if pings > 0 {
                state.pings.send_modify(|pinged| *pinged += pings);
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

#[derive(Default)]
struct Frames {
    upgraded: bool,
    unread: Vec<u8>,
}

impl Frames {
    fn pings_in(&mut self, bytes: &[u8]) -> u64 {
        self.unread.extend_from_slice(bytes);
        if !self.upgraded {
            let Some(end) = self
                .unread
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
            else {
                return 0;
            };
            self.unread.drain(..end + 4);
            self.upgraded = true;
        }
        let mut pings = 0;
        while let Some((payload, length)) = frame(&self.unread) {
            if matches!(
                be_protocol::decode::<be_protocol::ClientMessage>(&payload),
                Ok(be_protocol::ClientMessage::Ping { .. })
            ) {
                pings += 1;
            }
            self.unread.drain(..length);
        }
        pings
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
