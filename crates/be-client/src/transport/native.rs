use std::future::Future;

use be_protocol::MAX_FRAME_BYTES;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

use super::Frame;

type Stream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(crate) struct Writer(SplitSink<Stream, Message>);

pub(crate) struct Reader(SplitStream<Stream>);

pub(crate) fn spawn(future: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(future);
}

pub(crate) async fn connect(url: &str) -> Result<(Writer, Reader), String> {
    let (socket, _) = connect_async_with_config(
        url,
        Some(WebSocketConfig {
            max_frame_size: Some(MAX_FRAME_BYTES),
            max_message_size: Some(MAX_FRAME_BYTES),
            ..WebSocketConfig::default()
        }),
        false,
    )
    .await
    .map_err(|error| error.to_string())?;
    let (sink, source) = socket.split();
    Ok((Writer(sink), Reader(source)))
}

impl Writer {
    pub(crate) async fn send(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        self.send_message(Message::Binary(bytes)).await
    }

    pub(crate) async fn pong(&mut self, payload: Vec<u8>) -> Result<(), String> {
        self.send_message(Message::Pong(payload)).await
    }

    async fn send_message(&mut self, message: Message) -> Result<(), String> {
        self.0
            .send(message)
            .await
            .map_err(|error| error.to_string())
    }
}

impl Reader {
    pub(crate) async fn next(&mut self) -> Option<Frame> {
        loop {
            return match self.0.next().await? {
                Ok(Message::Binary(bytes)) => Some(Frame::Binary(bytes)),
                Ok(Message::Ping(payload)) => Some(Frame::Ping(payload)),
                Ok(Message::Text(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                Ok(Message::Close(_)) | Err(_) => Some(Frame::Closed),
            };
        }
    }
}
