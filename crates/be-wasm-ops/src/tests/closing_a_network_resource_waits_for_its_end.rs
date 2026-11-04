use super::*;

#[test]
fn closing_a_network_resource_waits_for_its_end() {
    let pipe = pipe();
    let reading = client::shelf::Reading::new(&pipe.client, "Book 5".to_owned());
    let embed::shelf::Call::ReadingNew { resource, .. } = pipe.accept_one() else {
        panic!("the server was not asked to open a reading");
    };
    let mut closing = Box::pin(reading.close());
    assert_eq!(poll(&mut closing), std::task::Poll::Pending);
    assert_eq!(pipe.pump(), vec![Incoming::Canceled { op: resource.op() }]);
    assert_eq!(poll(&mut closing), std::task::Poll::Pending);
    resource.end(&mut pipe.server.lock());
    pipe.pump();
    assert_eq!(poll(&mut closing), std::task::Poll::Ready(Ok(())));
}
