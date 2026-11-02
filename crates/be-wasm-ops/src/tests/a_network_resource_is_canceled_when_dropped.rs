use super::*;

#[test]
fn a_network_resource_is_canceled_when_dropped() {
    let pipe = pipe();
    let reading = client::shelf::Reading::new(&pipe.client, "Book 3".to_owned());
    let embed::shelf::Call::ReadingNew { resource, title } = pipe.accept_one() else {
        panic!("the server was not asked to open a reading");
    };
    assert_eq!((resource.op(), title.as_str()), (reading.op(), "Book 3"));

    let mut page = reading.page();
    let embed::shelf::Call::ReadingPage { this, reply } = pipe.accept_one() else {
        panic!("the server was not asked for a page");
    };
    assert_eq!(this, resource);
    reply.send(&mut pipe.server.lock(), &7);
    pipe.pump();
    assert_eq!(poll(&mut page), std::task::Poll::Ready(Ok(7)));

    let mut later = reading.page();
    let embed::shelf::Call::ReadingPage { reply, .. } = pipe.accept_one() else {
        panic!("the server was not asked for a page");
    };
    drop(reading);
    assert_eq!(pipe.pump(), vec![Incoming::Canceled { op: resource.op() }]);
    assert_eq!(
        poll(&mut pipe.server.canceled(resource.op())),
        std::task::Poll::Ready(())
    );
    resource.end(&mut pipe.server.lock());
    assert_eq!(
        poll(&mut pipe.server.canceled(reply.op())),
        std::task::Poll::Ready(())
    );
    reply.fail(&mut pipe.server.lock(), OpError::Canceled);
    pipe.pump();
    assert_eq!(
        poll(&mut later),
        std::task::Poll::Ready(Err(OpError::Canceled))
    );

    let mut ended = client::shelf::Reading::new(&pipe.client, "Book 4".to_owned());
    let embed::shelf::Call::ReadingNew { resource, .. } = pipe.accept_one() else {
        panic!("the server was not asked to open a reading");
    };
    assert_eq!(poll(&mut ended.closed()), std::task::Poll::Pending);
    resource.end(&mut pipe.server.lock());
    pipe.pump();
    assert_eq!(poll(&mut ended.closed()), std::task::Poll::Ready(Ok(())));
}
