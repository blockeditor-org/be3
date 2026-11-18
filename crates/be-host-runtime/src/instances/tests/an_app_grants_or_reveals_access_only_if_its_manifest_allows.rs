use super::*;
use be_sdk_api::{
    AccessLevel,
    embed::host_services::Call,
    ops::{OpError, Outcome},
};

/// An app grants or reveals access only if its manifest sets `sharing`: it cannot share the
/// block it shows, read who has it, or make or read its links unasked. Any app still opens a
/// settings kind of its own block to anyone inside, as the editor framework does, but opens
/// nothing else so, and names no one.
#[test]
fn an_app_grants_or_reveals_access_only_if_its_manifest_allows() {
    let own = Uuid::new_v4();
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
        ask(&mut instances, set(own, anyone_inside, AccessLevel::View)),
        denied,
        "opening its own block's settings kind to anyone inside was denied"
    );
    instances.allow_sharing(true);
    assert_ne!(
        ask(&mut instances, set(own, someone, AccessLevel::Edit)),
        denied,
        "an app whose manifest sets sharing was denied"
    );
}
