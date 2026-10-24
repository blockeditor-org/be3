use super::*;

const BUILTIN_TYPES: &[u8] = include_bytes!(env!("BUILTIN_TYPES_WASM"));

#[test]
fn the_apps_type_module_says_hello_on_a_thread_of_its_own() {
    let waiting = std::sync::Arc::default();
    let channel = PureHost::new(None)
        .unwrap()
        .spawn(
            "builtin-types".to_owned(),
            |host| host.load_bytes(BUILTIN_TYPES),
            std::sync::Arc::new(|| {}),
            std::sync::Arc::clone(&waiting),
        )
        .unwrap();
    let mut worker = block_guest_host::Worker::new(channel, waiting);
    let deadline = block_guest_host::Deadline::after(std::time::Duration::from_secs(60));
    while worker.take_received().is_empty() || !worker.ready() {
        assert!(
            worker.wait(deadline).expect("the module runs"),
            "the module never started"
        );
    }
    let entropy = block_type_api::Event::Entropy { seed: [3; 32] };
    let blank = block_type_api::Event::Request {
        id: 1,
        request: block_type_api::Request::Blank {
            content_type: uuid_of_counter(),
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

fn uuid_of_counter() -> uuid::Uuid {
    uuid::Uuid::from_u128(0x636f_756e_7465_722d_626c_6f63_6b2d_0001)
}
