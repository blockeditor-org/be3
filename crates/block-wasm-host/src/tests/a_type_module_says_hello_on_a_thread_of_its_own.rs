use super::*;

const COUNTER_TYPES: &[u8] = include_bytes!(env!("COUNTER_TYPES_WASM"));

#[test]
fn a_type_module_says_hello_on_a_thread_of_its_own() {
    let waiting = std::sync::Arc::default();
    let channel = PureHost::new(None)
        .unwrap()
        .spawn(
            "be3.counter".to_owned(),
            |host| host.load_bytes(COUNTER_TYPES),
            std::sync::Arc::new(|| {}),
            std::sync::Arc::clone(&waiting),
        )
        .unwrap();
    let mut worker = block_guest_host::Worker::new(channel, waiting);
    let deadline = block_guest_host::Deadline::after(std::time::Duration::from_secs(60));
    let mut said = Vec::new();
    while said.is_empty() || !worker.ready() {
        said.extend(worker.take_received());
        if !said.is_empty() && worker.ready() {
            break;
        }
        assert!(
            worker.wait(deadline).expect("the module runs"),
            "the module never started"
        );
    }
    let content_type = said
        .iter()
        .filter_map(|frame| block_type_api::decode_frame(frame).ok())
        .find_map(|command| match command {
            block_type_api::Command::Hello { types } => types.first().map(|info| info.content_type),
            _ => None,
        })
        .expect("the module says hello with the types it has");
    let entropy = block_type_api::Event::Entropy { seed: [3; 32] };
    let blank = block_type_api::Event::Request {
        id: 1,
        request: block_type_api::Request::Blank {
            content_type,
        },
    };
    assert_eq!(
        worker.queue(vec![
            block_type_api::encode_frame(&entropy),
            block_type_api::encode_frame(&blank),
        ]),
        None
    );
    loop {
        let replies: Vec<block_type_api::Command> = worker
            .take_received()
            .iter()
            .filter_map(|frame| block_type_api::decode_frame(frame).ok())
            .collect();
        if replies.iter().any(|reply| {
            matches!(
                reply,
                block_type_api::Command::Reply {
                    id: 1,
                    reply: block_type_api::Reply::Blank(_)
                }
            )
        }) {
            return;
        }
        assert!(
            worker.wait(deadline).expect("the module runs"),
            "the module never answered"
        );
    }
}
