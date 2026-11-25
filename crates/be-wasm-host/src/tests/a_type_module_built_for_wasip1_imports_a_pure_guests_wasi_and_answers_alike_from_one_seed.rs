use be_sdk_type_api::{
    embed::exports::replicas,
    ops::{Connection, Incoming, Side},
};

use super::*;

const COUNTER_TYPES: &[u8] = include_bytes!(env!("COUNTER_TYPES_WASM"));

/// Every frame the counter's module sends from its start until it has described its type's
/// blank content, run on a host seeded with `seed`.
fn answers(seed: [u8; 32]) -> Vec<Vec<u8>> {
    let waiting = std::sync::Arc::default();
    let channel = Host::pure()
        .unwrap()
        .with_seed(seed)
        .spawn(
            "be3.counter".to_owned(),
            |host| host.load_bytes(COUNTER_TYPES),
            std::sync::Arc::new(|| {}),
            std::sync::Arc::clone(&waiting),
            be_wasm_guest_host::thread::catch,
        )
        .unwrap();
    let mut worker = be_wasm_guest_host::Worker::new(channel, waiting);
    let deadline = be_wasm_guest_host::Deadline::after(std::time::Duration::from_secs(60));
    let mut host = Connection::new(Side::Host);
    let started = replicas::start(&mut host);
    assert_eq!(worker.queue(host.take_outbound()), None);
    let (mut sent, mut described) = (Vec::new(), None);
    loop {
        for frame in worker.take_received() {
            host.receive(&frame).unwrap();
            sent.push(frame);
        }
        while let Some(event) = host.next_event() {
            match event {
                Incoming::Completed { op, outcome } if op == started.op() => {
                    let types = started.decode(outcome).expect("the module lists its types");
                    let info = types.first().expect("it has a type");
                    described = Some(replicas::describe(
                        &mut host,
                        info.content_type,
                        info.blank.clone(),
                        uuid::Uuid::nil(),
                    ));
                    assert_eq!(worker.queue(host.take_outbound()), None);
                }
                Incoming::Completed { op, .. }
                    if described.is_some_and(|described| described.op() == op) =>
                {
                    return sent;
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

#[test]
fn a_type_module_built_for_wasip1_imports_a_pure_guests_wasi_and_answers_alike_from_one_seed() {
    for import in wasmparser::Parser::new(0)
        .parse_all(COUNTER_TYPES)
        .filter_map(|payload| match payload.unwrap() {
            wasmparser::Payload::ImportSection(reader) => Some(reader),
            _ => None,
        })
        .flat_map(|reader| reader.into_imports())
    {
        let import = import.unwrap();
        assert!(
            import.module == "be3_host" || be_wasm_pure_wasi::answers(import.module, import.name),
            "the type module imports {}.{}",
            import.module,
            import.name
        );
    }
    assert_eq!(answers([7; 32]), answers([7; 32]));
}
