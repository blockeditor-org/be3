use std::time::Duration;

use futures_util::future::{Either, select};
use futures_util::pin_mut;

use crate::dbus::test_bus::Bus;

struct Echo;

#[zbus::interface(name = "org.be3.Echo")]
impl Echo {
    fn echo(&self, text: String) -> String {
        text
    }
}

#[test]
fn an_object_served_after_connecting_answers_the_first_call() {
    let bus = Bus::start();
    zbus::block_on(async {
        let server = crate::dbus::build(bus.builder().internal_executor(false))
            .await
            .expect("the bus takes a connection");
        server
            .object_server()
            .at("/echo", Echo)
            .await
            .expect("the object is served");
        let client = bus
            .builder()
            .method_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("the bus takes a connection");
        let call = client.call_method(
            Some(server.unique_name().expect("a name").as_str()),
            "/echo",
            Some("org.be3.Echo"),
            "Echo",
            &("hello",),
        );
        let serving = async {
            loop {
                server.executor().tick().await;
            }
        };
        pin_mut!(call, serving);
        let Either::Left((reply, _)) = select(call, serving).await;
        let reply = reply.expect("the object answers its first call");
        assert_eq!(
            reply.body().deserialize::<String>().expect("a string"),
            "hello"
        );
    });
}
