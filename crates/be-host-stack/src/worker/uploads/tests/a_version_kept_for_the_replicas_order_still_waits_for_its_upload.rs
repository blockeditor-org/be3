use super::*;

use crate::worker::{Served, admit};

/// A version command waits for the replicas' order first (`after_order`), and is then admitted
/// as any other: one on a block still uploading waits for the upload, not only for the order.
#[test]
fn a_version_kept_for_the_replicas_order_still_waits_for_its_upload() {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let mut uploads = Uploads::new(&shared);
    let mut served = Served::default();
    let checkout = Uuid::new_v4();
    uploads.add(vec![(
        Saved {
            id: checkout,
            block_type: Uuid::nil(),
            parent: None,
            metadata: be_block_content::BlockMetadata::default(),
            references: Vec::new(),
            content: None,
            log: Vec::new(),
        },
        BlockParent::Root,
    )]);
    let version = Command::Version {
        block: checkout,
        command: be_sdk_api::VersionCommand::CreateBranch {
            name: "draft".into(),
        },
    };
    assert!(
        admit(&mut uploads, &mut served, version).is_none(),
        "a version command on a block still uploading waits for it"
    );
    assert_eq!(uploads.deferred(), 1);
    assert!(uploads.runnable().is_empty());

    uploads.queue.borrow_mut().finish(checkout);
    let runnable: Vec<&'static str> = uploads.runnable().iter().map(Command::name).collect();
    assert_eq!(runnable, ["version"], "it runs once the block is uploaded");
    assert!(uploads.is_empty());
}
