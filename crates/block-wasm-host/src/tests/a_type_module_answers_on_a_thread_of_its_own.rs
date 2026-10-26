use block_type_api::{
    embed::exports::replicas,
    ops::{Connection, Incoming, Side},
};

use super::*;

const COUNTER_TYPES: &[u8] = include_bytes!(env!("COUNTER_TYPES_WASM"));

#[test]
fn a_type_module_answers_on_a_thread_of_its_own() {
    let waiting = std::sync::Arc::default();
    let channel = PureHost::new(None)
        .unwrap()
        .with_imports(block_type_api::embed::IMPORTS)
        .spawn(
            "be3.counter".to_owned(),
            |host| host.load_bytes(COUNTER_TYPES),
            std::sync::Arc::new(|| {}),
            std::sync::Arc::clone(&waiting),
        )
        .unwrap();
    let mut worker = block_guest_host::Worker::new(channel, waiting);
    let deadline = block_guest_host::Deadline::after(std::time::Duration::from_secs(60));
    while !worker.ready() {
        assert!(
            worker.wait(deadline).expect("the module runs"),
            "the module never started"
        );
    }
    let mut host = Connection::new(Side::Host);
    let started = replicas::start(&mut host, vec![3; 32]);
    assert_eq!(worker.queue(host.take_outbound()), None);
    let mut blank = None;
    loop {
        for frame in worker.take_received() {
            host.receive(&frame).unwrap();
        }
        while let Some(event) = host.next_event() {
            match event {
                Incoming::Completed { op, outcome } if op == started.op() => {
                    let types = started.decode(outcome).expect("the module lists its types");
                    let content_type = types.first().expect("it has a type").content_type;
                    blank = Some(replicas::blank(&mut host, content_type));
                    assert_eq!(worker.queue(host.take_outbound()), None);
                }
                Incoming::Completed { op, outcome }
                    if blank.is_some_and(|blank| blank.op() == op) =>
                {
                    let answered = blank.unwrap().decode(outcome);
                    assert!(matches!(answered, Ok(Ok(_))), "{answered:?}");
                    return;
                }
                _ => {}
            }
        }
        assert!(
            worker.wait(deadline).expect("the module runs"),
            "the module never answered"
        );
    }
}
