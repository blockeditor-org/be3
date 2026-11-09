use super::*;
use be_sdk_api::{
    embed::host_services::Call,
    ops::{OpError, Outcome},
};

#[test]
fn an_app_makes_a_blocks_agent_only_if_its_manifest_allows_and_only_for_its_own_block() {
    let own = Uuid::new_v4();
    let elsewhere = Uuid::new_v4();
    let mut instances = placed_on(own, Uuid::new_v4());
    let ask = |instances: &mut Instances, call: Call| {
        instances.take_answers();
        instances.serve(crate::wire::Service {
            instance: INSTANCE,
            call,
        });
        instances.take_answers()
    };
    let denied = vec![(1, Outcome::Err(OpError::Denied))];
    for call in [
        Call::CreateBlockAgent {
            reply: reply(1),
            block: own.into_bytes(),
            name: "boat VM".into(),
        },
        Call::RotateAgentToken {
            reply: reply(1),
            agent: Uuid::new_v4().into_bytes(),
        },
        Call::RemoveAgent {
            reply: reply(1),
            agent: Uuid::new_v4().into_bytes(),
        },
    ] {
        assert_eq!(
            ask(&mut instances, call),
            denied,
            "an app whose manifest does not set block_agents was not denied"
        );
    }

    instances.allow_block_agents(true);
    let outside = Call::CreateBlockAgent {
        reply: reply(1),
        block: elsewhere.into_bytes(),
        name: "boat VM".into(),
    };
    assert_eq!(
        ask(&mut instances, outside),
        denied,
        "an app made an agent for a block outside its own"
    );
}
