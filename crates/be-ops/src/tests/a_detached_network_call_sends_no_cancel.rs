use super::*;

#[test]
fn a_detached_network_call_sends_no_cancel() {
    let pipe = pipe();
    let held = client::shelf::hold(&pipe.client, "Book 1".to_owned());
    let op = held.op();
    held.detach();
    let embed::shelf::Call::Hold { reply, .. } = pipe.accept_one() else {
        panic!("the server was not asked to hold a book");
    };
    reply.send(&mut pipe.server.lock(), &());
    assert_eq!(pipe.pump(), vec![], "nothing was canceled");
    assert!(!pipe.client.lock().is_pending(op));
}
