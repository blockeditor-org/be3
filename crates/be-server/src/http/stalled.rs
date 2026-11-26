//! A deadline on writing: hyper writes an answer for as long as the client takes to read it, so
//! a client that stops reading a large answer (an S3 object, a long-poll's) would hold its
//! connection and place until it went. `WriteDeadline` fails a write that has made no progress
//! for its deadline, which ends the connection; a connection with nothing to write, or a write
//! that keeps moving however slowly, is never cut by it.

use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

pub(crate) struct WriteDeadline<T> {
    inner: T,
    deadline: Duration,
    /// Set while a write waits: when it is given up unless some of it goes before.
    stalled: Option<Pin<Box<Sleep>>>,
}

impl<T> WriteDeadline<T> {
    pub(crate) fn new(inner: T, deadline: Duration) -> Self {
        Self {
            inner,
            deadline,
            stalled: None,
        }
    }

    /// What a write that waits answers: still waiting, or timed out once it waited the deadline.
    fn waiting<R>(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<R>> {
        let deadline = self.deadline;
        let stalled = self
            .stalled
            .get_or_insert_with(|| Box::pin(tokio::time::sleep(deadline)));
        match stalled.as_mut().poll(cx) {
            Poll::Ready(()) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the client read none of the answer for too long",
            ))),
            Poll::Pending => Poll::Pending,
        }
    }

    /// What a write that went answers, which starts the deadline over.
    fn went<R>(&mut self, result: Poll<io::Result<R>>) -> Poll<io::Result<R>> {
        self.stalled = None;
        result
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for WriteDeadline<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for WriteDeadline<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_write(cx, buf) {
            Poll::Pending => this.waiting(cx),
            went => this.went(went),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_write_vectored(cx, bufs) {
            Poll::Pending => this.waiting(cx),
            went => this.went(went),
        }
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_flush(cx) {
            Poll::Pending => this.waiting(cx),
            went => this.went(went),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_shutdown(cx) {
            Poll::Pending => this.waiting(cx),
            went => this.went(went),
        }
    }
}
