use super::*;

#[test]
fn dropping_a_network_call_cancels_it() {
    let pipe = pipe();
    let held = client::shelf::hold(&pipe.client, "Book 1".to_owned());
    let op = held.op();
    let embed::shelf::Call::Hold { reply, title } = pipe.accept_one() else {
        panic!("the server was not asked to hold a book");
    };
    assert_eq!((reply.op(), title.as_str()), (op, "Book 1"));
    assert_eq!(
        poll(&mut pipe.server.canceled(op)),
        std::task::Poll::Pending
    );

    drop(held);
    assert_eq!(pipe.pump(), vec![Incoming::Canceled { op }]);
    assert_eq!(
        poll(&mut pipe.server.canceled(op)),
        std::task::Poll::Ready(())
    );
    reply.fail(&mut pipe.server.lock(), OpError::Canceled);
    assert_eq!(pipe.pump(), vec![]);
    assert!(!pipe.client.lock().is_pending(op));
}
