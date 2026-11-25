use be_sdk_type_api::{
    Plain, TypeInfo,
    ops::{Connection, Incoming, Outcome, Side, encode, method_id},
};
use be_wasm_guest_abi::{Flow, Guest, Waker};

use super::*;
use crate::Modules;

const START: &str = "be3:block-type/replicas@0.1.0#start";
const ERROR: &str = "be3:block-type/host@0.1.0#error";
const TYPE: Uuid = Uuid::from_u128(0x7a1c);

/// A module that, as it answers the host's start, also reports an error: two things said in
/// one step, of which a wait for its hello takes only the hello.
struct Talker {
    connection: Connection,
}

impl Guest for Talker {
    fn start(_: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            connection: Connection::new(Side::Guest),
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        for frame in &inbound {
            self.connection.receive(frame).unwrap();
        }
        while let Some(event) = self.connection.next_event() {
            if let Incoming::Started(started) = event
                && started.method == method_id(START)
            {
                let types = vec![TypeInfo {
                    content_type: TYPE,
                    frozen: false,
                    schema: String::new(),
                    undo: false,
                    plain: Plain::None,
                    first_part: false,
                    merges_parts: false,
                    log: false,
                    blank: Vec::new(),
                    blank_description: Description::default(),
                    reads: None,
                }];
                self.connection
                    .complete(started.op, Outcome::returned(&types));
                let _ = self.connection.start(
                    None,
                    method_id(ERROR),
                    encode(&"said while the host waited".to_owned()),
                );
            }
        }
        outbound.extend(self.connection.take_outbound());
        Ok(Flow::Running)
    }
}

/// What a module says while the host waits on it for something else is kept for the next pump,
/// and the wake that told of it went to that wait: the host's next wait for the modules ends at
/// once rather than leaving it until the module next speaks.
#[test]
fn what_a_module_says_during_a_wait_on_it_is_taken_without_another_wake() {
    be_lib_build_id::install("test");
    let mut modules = Modules::new(
        vec![in_process::<Talker>("talker").declaring(vec![TYPE])],
        be_block_client::Clock::real(),
    );
    futures_util::FutureExt::now_or_never(modules.load(0)).expect("the module said hello");
    assert!(
        modules.kept(),
        "the wait for the hello kept the error for later"
    );
    // Whatever wake the module gave went to that wait.
    while futures_util::FutureExt::now_or_never(modules.notified()).is_some() {}

    let mut replicas = Replicas::<Quiet>::new(modules);
    assert!(
        futures_util::FutureExt::now_or_never(replicas.woken()).is_some(),
        "the next wait for the modules ends at once, to take what was kept"
    );
}
