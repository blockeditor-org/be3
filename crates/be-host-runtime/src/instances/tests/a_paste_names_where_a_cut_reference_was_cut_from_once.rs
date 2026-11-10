use super::*;

use be_block::{BlockContent, BlockMetadata};
use be_block_graph::BlockParent;
use be_sdk_api::embed::host_services::Call as Service;
use be_sdk_api::{BlockLocation, Failure, PastedBlock};
use be_type_database::database_view::{DatabaseView, DatabaseViewContent};
use be_type_example_counter::CounterContent;

fn made(block: Uuid) {
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
        false,
    );
}

fn ask(instances: &mut Instances, call: Service) -> Vec<(OpId, be_sdk_api::ops::Outcome)> {
    instances.take_answers();
    instances.serve(crate::wire::Service {
        instance: INSTANCE,
        call,
    });
    instances.take_answers()
}

fn cut(instances: &mut Instances, block: Uuid, holder: Uuid) {
    ask(
        instances,
        Service::CutBlocks {
            reply: reply(next_op()),
            blocks: vec![block.into_bytes()],
            holder: holder.into_bytes(),
        },
    );
}

/// Pastes `block` into `folder`, answering where its cut was from.
fn paste(instances: &mut Instances, block: Uuid, folder: Uuid) -> Option<Uuid> {
    let answers = ask(
        instances,
        Service::PasteBlocks {
            reply: reply(next_op()),
            blocks: vec![block.into_bytes()],
            parent: BlockLocation::Block(folder.into_bytes()),
        },
    );
    let [(_, outcome)]: [_; 1] = answers.try_into().expect("the paste is answered once");
    let pasted = outcome
        .decode::<Result<Vec<PastedBlock>, Failure>>()
        .expect("the answer decodes")
        .expect("the paste is allowed");
    let [pasted]: [_; 1] = pasted.try_into().expect("the block is answered");
    assert!(!pasted.moved, "a reference is not moved by the host");
    pasted.cut_from.map(Uuid::from_bytes)
}

#[test]
fn a_paste_names_where_a_cut_reference_was_cut_from_once() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until(|_| {
        (be_host_stack::is_known(DatabaseViewContent::CONTENT_TYPE)
            && be_host_stack::is_known(CounterContent::CONTENT_TYPE))
        .then_some(())
    });
    let (holder, linked, folder) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    made(linked);
    made(folder);
    let mut instances = placed_on(holder, DatabaseViewContent::CONTENT_TYPE);
    call(
        &mut instances,
        Call::WriteContent {
            reply: reply(next_op()),
            block: holder.into_bytes(),
            content_type: DatabaseViewContent::CONTENT_TYPE.into_bytes(),
            bytes: DatabaseViewContent::new(&DatabaseView::of(linked)).encode(),
            replace: false,
        },
    );
    be_host_stack::flush();
    to_replica(
        &mut instances,
        holder,
        &be_sdk_type_guest::editor::FromEditor::Resend,
    );
    instances.next_screens(PASS);
    be_host_stack::flush();
    be_host_stack::wait_until(|shared| {
        let node = shared.graph.get(holder)?;
        (node.references == [linked]).then_some(())
    });
    be_host_stack::set_parent(holder, BlockParent::Root);

    assert_eq!(paste(&mut instances, linked, folder), None, "nothing was cut");

    cut(&mut instances, linked, holder);
    assert_eq!(
        paste(&mut instances, linked, folder),
        Some(holder),
        "the first paste after the cut names the block the reference was cut from"
    );
    assert_eq!(
        paste(&mut instances, linked, folder),
        None,
        "that paste used the cut up, so the next one links"
    );

    cut(&mut instances, linked, folder);
    assert_eq!(
        paste(&mut instances, linked, folder),
        None,
        "a block that does not hold the reference is not named"
    );

    cut(&mut instances, linked, holder);
    be_host_stack::set_parent(holder, BlockParent::Detached);
    assert_eq!(
        paste(&mut instances, linked, folder),
        None,
        "nor is a deleted one, so the paste links"
    );
}
