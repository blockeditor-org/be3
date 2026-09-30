use std::{
    io::{self, Write},
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll},
};

use bytes::Bytes;
use tokio::io::AsyncWrite;
use wasmtime_wasi::{
    async_trait,
    cli::{IsTerminal, StdoutStream},
    p2::{OutputStream, Pollable, StreamError},
};

#[derive(Clone, Default)]
pub struct Tee {
    written: Arc<Mutex<Vec<u8>>>,
}

impl Tee {
    pub fn contents(&self) -> String {
        let written = self.written.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&written).into_owned()
    }

    fn write(&self, bytes: &[u8]) {
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(bytes);
        let _ = stderr.flush();
        self.written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
    }
}

impl IsTerminal for Tee {
    fn is_terminal(&self) -> bool {
        io::IsTerminal::is_terminal(&io::stderr())
    }
}

impl StdoutStream for Tee {
    fn p2_stream(&self) -> Box<dyn OutputStream> {
        Box::new(self.clone())
    }

    fn async_stream(&self) -> Box<dyn AsyncWrite + Send + Sync> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl OutputStream for Tee {
    fn write(&mut self, bytes: Bytes) -> Result<(), StreamError> {
        Tee::write(self, &bytes);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), StreamError> {
        Ok(())
    }

    fn check_write(&mut self) -> Result<usize, StreamError> {
        Ok(usize::MAX)
    }
}

#[async_trait]
impl Pollable for Tee {
    async fn ready(&mut self) {}
}

impl AsyncWrite for Tee {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.write(bytes);
        Poll::Ready(Ok(bytes.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
