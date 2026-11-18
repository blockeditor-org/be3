use be_host_base::host_state::HostStateStore;

use super::*;
use crate::device_push::{self, Shown};

/// While the stack runs in this process in a push's organization, the push opens through it,
/// named from the stack's contacts and graph: a contact's message about a chat this device
/// holds.
#[test]
fn a_push_opens_through_the_stack_running_in_its_organization() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let ada = Member::join(&harness, personal, "ada@example.com", "Ada");
    let block = Uuid::new_v4();
    create(
        block,
        be_type_text::TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Plans"),
        Some(be_type_text::TextBlock::of("Lunch?").encode()),
        false,
    );
    flush();
    set_access(block, ada.account(), be_block_graph::Access::Edit, |_| {});
    flush_and_send();
    assert_eq!(status().error, None);
    let now = be_block_commit::now_milliseconds();
    let sealed = ada
        .runtime
        .block_on(ada.peer.seal_notification(&be_block_client::Notification {
            organization: personal,
            block,
            sender: ada.account(),
            sent: now,
            text: "Noon?".to_owned(),
        }))
        .expect("Ada seals the notification with the block's key");
    let message = be_block_protocol::PushMessage {
        organization: personal,
        block,
        sender: ada.account(),
        sealed,
    };

    let state = HostStateStore::open(harness.directory.join("host.sqlite3")).expect("it opens");
    let keys = device_push::keys(&state).expect("the device makes its push keys");
    let body = be_lib_web_push::encrypt(&keys.public(), &keys.auth(), &message.encode(), 0)
        .expect("the push encrypts");
    // The refs database this directory holds has nothing of the account's: only the stack
    // can open it.
    let elsewhere = harness.directory.join("elsewhere");
    assert_eq!(crate::running_organization(), Some(personal));
    assert_eq!(
        device_push::receive(&state, &elsewhere, &body, now),
        Some(Shown {
            title: "Plans".to_owned(),
            body: "Ada: Noon?".to_owned(),
            opens: Some((personal, block)),
        })
    );
}
