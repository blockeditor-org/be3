use std::future::Future;

use super::*;

#[test]
fn a_network_stream_grants_credit_as_it_is_read() {
    let pipe = pipe();
    let mut books = client::shelf::books(&pipe.client, 0);
    let embed::shelf::Call::Books { stream, first: 0 } = pipe.accept_one() else {
        panic!("the server was not asked for books");
    };
    let mut sent = 0;
    let send_while_writable = |sent: &mut u32| {
        while poll(&mut pipe.server.writable(stream.op())) == std::task::Poll::Ready(Ok(())) {
            stream.send(&mut pipe.server.lock(), &book(*sent)).unwrap();
            *sent += 1;
        }
    };
    send_while_writable(&mut sent);
    assert_eq!(sent, STREAM_WINDOW);
    pipe.pump();
    let woken = std::sync::Arc::new(Woken::default());
    let waker = std::task::Waker::from(std::sync::Arc::clone(&woken));
    let mut writable = pipe.server.writable(stream.op());
    let polled =
        std::pin::Pin::new(&mut writable).poll(&mut std::task::Context::from_waker(&waker));
    assert_eq!(polled, std::task::Poll::Pending);

    for pages in 0..STREAM_WINDOW / 2 - 1 {
        assert_eq!(
            poll(&mut books.recv()),
            std::task::Poll::Ready(Some(book(pages)))
        );
    }
    pipe.pump();
    send_while_writable(&mut sent);
    assert_eq!(sent, STREAM_WINDOW);

    assert_eq!(
        poll(&mut books.recv()),
        std::task::Poll::Ready(Some(book(STREAM_WINDOW / 2 - 1)))
    );
    assert!(!woken.0.load(std::sync::atomic::Ordering::SeqCst));
    pipe.pump();
    assert!(woken.0.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(poll(&mut writable), std::task::Poll::Ready(Ok(())));
    send_while_writable(&mut sent);
    assert_eq!(sent, STREAM_WINDOW + STREAM_WINDOW / 2);

    stream.finish(&mut pipe.server.lock());
    pipe.pump();
    for pages in STREAM_WINDOW / 2..sent {
        assert_eq!(
            poll(&mut books.recv()),
            std::task::Poll::Ready(Some(book(pages)))
        );
    }
    assert_eq!(poll(&mut books.recv()), std::task::Poll::Ready(None));
    assert_eq!(books.ended(), Some(Ok(())));
}

#[derive(Default)]
struct Woken(std::sync::atomic::AtomicBool);

impl std::task::Wake for Woken {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
