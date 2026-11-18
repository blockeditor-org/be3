use super::*;
use be_type_example_counter::CounterContent;
use be_sdk_api::{
    AccessLevel,
    embed::host_services::Call,
    ops::{OpError, Outcome},
};

/// An app grants or reveals access only if its manifest sets `sharing`: it cannot share the
/// block it shows, open it to anyone inside or close it, read who has it, or make or read its
/// links unasked. Any app still opens a block it just made to anyone inside, as the editor
/// framework does for a settings kind, but opens nothing else so, and names no one.
#[test]
fn an_app_grants_or_reveals_access_only_if_its_manifest_allows() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until("knew the counter type", |_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let own = Uuid::new_v4();
    let made = Uuid::now_v7();
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    call(
        &mut instances,
        be_sdk_api::embed::blocks::Call::CreateBlock {
            reply: reply(next_op()),
            block: made.into_bytes(),
            block_type: CounterContent::CONTENT_TYPE.into_bytes(),
            parent: be_sdk_api::BlockLocation::Root,
            defined: Vec::new(),
            content: None,
            owner_only: false,
        },
    );
    be_host_stack::flush_and_send();
    let ask = |instances: &mut Instances, call: Call| {
        instances.take_answers();
        instances.serve(crate::wire::Service {
            instance: INSTANCE,
            call,
        });
        instances.take_answers()
    };
    let denied = vec![(1, Outcome::Err(OpError::Denied))];
    let someone = Uuid::new_v4().into_bytes();
    let anyone_inside = be_block_graph::ANYONE_INSIDE.into_bytes();
    let set = |block: Uuid, account: [u8; 16], access| Call::SetAccess {
        reply: reply(1),
        block: block.into_bytes(),
        account,
        access,
    };
    for call in [
        set(own, someone, AccessLevel::Edit),
        set(own, anyone_inside, AccessLevel::Edit),
        set(own, anyone_inside, AccessLevel::View),
        set(own, anyone_inside, AccessLevel::None),
        set(Uuid::new_v4(), anyone_inside, AccessLevel::View),
        Call::ListAccess {
            reply: reply(1),
            block: own.into_bytes(),
        },
        Call::TransferOwnership {
            reply: reply(1),
            block: own.into_bytes(),
            account: someone,
        },
        Call::ListShareLinks {
            reply: reply(1),
            block: own.into_bytes(),
        },
        Call::MakeShareLink {
            reply: reply(1),
            block: own.into_bytes(),
            access: AccessLevel::View,
            admits: be_sdk_api::LinkAdmits::Members,
        },
        Call::RevokeShareLink {
            reply: reply(1),
            block: own.into_bytes(),
            link: "link".into(),
        },
        Call::MakeContactLink {
            reply: reply(1),
            block: own.into_bytes(),
            access: AccessLevel::View,
        },
    ] {
        let name = format!("{call:?}");
        assert_eq!(
            ask(&mut instances, call),
            denied,
            "an app whose manifest does not set sharing was not denied {name}"
        );
    }
    assert_ne!(
        ask(&mut instances, set(made, anyone_inside, AccessLevel::View)),
        denied,
        "opening a block it just made to anyone inside was denied"
    );
    instances.allow_sharing(true);
    assert_ne!(
        ask(&mut instances, set(own, someone, AccessLevel::Edit)),
        denied,
        "an app whose manifest sets sharing was denied"
    );
}
