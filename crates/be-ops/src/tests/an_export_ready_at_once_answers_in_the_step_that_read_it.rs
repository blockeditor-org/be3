use super::*;

#[test]
fn an_export_ready_at_once_answers_in_the_step_that_read_it() {
    let executor = guest::Executor::new(Box::new(|_, _| {
        guest::Serve::Later(Box::pin(std::future::ready(Outcome::returned(&7u32))))
    }));
    let mut host = Connection::new(Side::Host);
    let asked = host.start(None, method_id("test#count"), encode(&()));
    let mut outbound = Vec::new();
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    for frame in outbound {
        host.receive(&frame).unwrap();
    }
    assert_eq!(
        events(&mut host),
        vec![Incoming::Completed {
            op: asked,
            outcome: Outcome::returned(&7u32),
        }]
    );
}
