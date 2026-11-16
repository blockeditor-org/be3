use super::*;
use be_sdk_api::ops::{OpError, Outcome};

/// A host component runs unattended, so the host keeps it to its own block and the blocks below
/// it and refuses it whatever would ask a person (guides/apps/host_services.md, "Host
/// components"), whatever the app asks.
#[test]
fn a_host_component_reaches_only_its_own_block_and_asks_no_one() {
    let own = Uuid::new_v4();
    let elsewhere = Uuid::new_v4();
    let mut instances = Instances::default();
    instances.report_creation(
        INSTANCE,
        Uuid::nil(),
        &Arc::new(crate::AppCatalog::default()),
        InstanceRole::Host(
            Uuid::new_v4(),
            EditorBlock {
                id: own,
                block_type: Uuid::new_v4(),
                view_block: None,
            },
        ),
    );
    let denied = vec![(1, Outcome::Err(OpError::Denied))];
    let graph = |instances: &mut Instances, call: Call| {
        instances.take_answers();
        instances.blocks_call(INSTANCE, call);
        instances.take_answers()
    };
    let delete = |source: Uuid| Call::Act {
        reply: reply(1),
        block: source.into_bytes(),
        command: be_sdk_api::BlockCommand::Delete {
            block_type: [0; 16],
            source: be_sdk_api::BlockLocation::Block(source.into_bytes()),
        },
    };
    assert_eq!(graph(&mut instances, delete(elsewhere)), denied, "a block outside its own");
    assert_eq!(
        graph(
            &mut instances,
            Call::CreateBlock {
                reply: reply(1),
                block: Uuid::now_v7().into_bytes(),
                block_type: [0; 16],
                parent: be_sdk_api::BlockLocation::Root,
                defined: Vec::new(),
                content: None,
                owner_only: false,
            },
        ),
        denied,
        "a block made at the root"
    );
    assert_ne!(
        graph(&mut instances, delete(own)),
        denied,
        "its own block is within its reach"
    );

    instances.take_answers();
    instances.serve(crate::wire::Service {
        instance: INSTANCE,
        call: be_sdk_api::embed::host_services::Call::PickFile {
            reply: reply(1),
            filter: Default::default(),
        },
    });
    assert_eq!(instances.take_answers(), denied, "a file picked by a person");
}
