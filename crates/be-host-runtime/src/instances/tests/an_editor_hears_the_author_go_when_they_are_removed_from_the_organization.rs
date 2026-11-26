use super::*;

use be_sdk_api::blocks::BlockSubject;
use be_type_example_counter::CounterContent;

#[test]
fn an_editor_hears_the_author_go_when_they_are_removed_from_the_organization() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let ada = harness.server().account("Ada");
    let ada = harness.invite(
        be_host_stack::testing::SignIn {
            url: harness.server().url(),
            account: ada.id,
            token: harness.server().token(ada.id),
            email: ada.email,
        },
        "Ada",
    );
    // Ada makes a block of her own and shares it with nobody: this account, an administrator,
    // may view it all the same. She views it as its author, holding no grant on it whose
    // revoke would move its change, and no key of it is rotated as she goes, since nobody she
    // shared it with lost it.
    let block = ada.make(
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
    );
    // The stack lists the roots again as it connects, hers among them. The mirror it kept may
    // hold her block before the connection is back, and the server names the members only
    // once it is (an ask made offline fails), so the wait is for both.
    harness.connect();
    let shared = be_host_stack::wait_until("Ada's block is in the mirror, connected", |shared| {
        shared
            .graph
            .get(block)
            .filter(|node| shared.connected && node.access.can_view())
            .map(|node| node.changed)
    });

    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    let watch = open_watch(&mut instances, |op| Call::BlockWatchNew {
        resource: wire::BlockWatch(op),
        subject: BlockSubject::Members,
    });
    // An administrator is not named: its view comes of its role, not of the block.
    assert_eq!(
        accounts(members(&mut instances, watch).expect("the server named the members")),
        [ada.account()]
    );

    // The worker's answer reaches the reply as the stack's updates are taken.
    let removed = Arc::new(std::sync::Mutex::new(None));
    let answer = Arc::clone(&removed);
    be_host_stack::remove_member(
        ada.account(),
        Box::new(move |listed| {
            *answer.lock().unwrap() = Some(listed.map(|_| ()));
        }),
    );
    let answered = || removed.lock().unwrap().is_some();
    while !answered() {
        be_host_stack::take_updates_or_wait("Ada's removal", answered);
    }
    let removed = removed.lock().unwrap().take();
    removed
        .expect("the worker answered")
        .expect("Ada is removed from the organization");
    // Her removal moves the block's change, though nothing of it was revoked or rotated.
    let taken = || be_host_stack::node(block).is_some_and(|node| node.changed > shared);
    while !taken() {
        be_host_stack::take_updates_or_wait("the block changing as Ada leaves", taken);
    }

    // An answer to an ask made before it may still come first; the ask the change makes
    // follows it, and names nobody. Each wait is bounded, so one that never comes fails.
    while !accounts(members(&mut instances, watch).expect("the server named the members"))
        .is_empty()
    {}
}
