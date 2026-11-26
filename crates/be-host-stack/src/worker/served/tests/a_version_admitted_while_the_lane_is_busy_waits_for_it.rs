use super::*;

use crate::worker::{Uploads, admit};

/// A version command may reach a whole tree, so one admitted while the lane runs a command waits
/// for the lane to be empty, as after the replicas' order (`after_order`) as anywhere else.
#[test]
fn a_version_admitted_while_the_lane_is_busy_waits_for_it() {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let mut uploads = Uploads::new(&shared);
    let mut served = Served::default();
    served.in_lane.set(1);
    let version = Command::Version {
        block: Uuid::new_v4(),
        command: be_sdk_api::VersionCommand::CreateBranch {
            name: "draft".into(),
        },
    };
    assert!(
        admit(&mut uploads, &mut served, version).is_none(),
        "a version command waits while the lane runs a command"
    );
    assert_eq!(served.deferred(), 1);
    assert!(served.runnable().is_empty());

    served.in_lane.set(0);
    let order: Vec<&'static str> = served.runnable().iter().map(Command::name).collect();
    assert_eq!(order, ["version"], "it runs once the lane is empty");
}
