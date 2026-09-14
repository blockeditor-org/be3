#[cfg(all(target_arch = "wasm32", feature = "hosted"))]
mod hosted;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod tunnel;
#[cfg(all(target_arch = "wasm32", not(feature = "hosted")))]
mod web;

#[cfg(all(target_arch = "wasm32", feature = "hosted"))]
pub use hosted::pump;
#[cfg(all(target_arch = "wasm32", feature = "hosted"))]
pub(crate) use hosted::{Socket, post_json, spawn_worker};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{Socket, post_json, spawn_worker};
pub(crate) use tunnel::TunnelSocket;
#[cfg(all(target_arch = "wasm32", not(feature = "hosted")))]
pub(crate) use web::{Socket, post_json, spawn_worker};

pub(crate) enum Link {
    Socket(Socket),
    Tunnel(TunnelSocket),
}

impl Link {
    pub(crate) async fn send_text(&mut self, text: String) -> Result<(), String> {
        match self {
            Self::Socket(socket) => socket.send_text(text).await,
            Self::Tunnel(tunnel) => tunnel.send_text(text).await,
        }
    }

    pub(crate) async fn next(&mut self) -> Option<Result<SocketMessage, String>> {
        match self {
            Self::Socket(socket) => socket.next().await,
            Self::Tunnel(tunnel) => tunnel.next().await,
        }
    }
}

pub(crate) enum SocketMessage {
    Text(String),

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Ping(usize),
    Close,
}
