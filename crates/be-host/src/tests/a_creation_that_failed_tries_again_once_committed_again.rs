use be_host_runtime::ChildCommit;
use be_sdk_api::{BlockLocation, ChildId, CreationProgress};

use crate::editors::step_creation;

#[test]
fn a_creation_that_failed_tries_again_once_committed_again() {
    let commit = || ChildCommit {
        child: ChildId(1),
        parent: BlockLocation::Block([7; 16]),
        name: None,
    };
    let mut held = None;
    let mut progress = None;

    // Uncommitted, it offers its options.
    assert!(step_creation("Canvas", &mut held, &mut progress, || {
        unreachable!("nothing is made before a commit")
    }));
    assert_eq!(progress, Some(CreationProgress::Options));

    // Committed, its block could not be made: it says why, in both hosts' words, and lets go
    // of the commit.
    held = Some(commit());
    assert!(step_creation("Canvas", &mut held, &mut progress, || Err(
        "the app stopped".to_owned()
    )));
    assert_eq!(
        progress,
        Some(CreationProgress::Failed(
            "Canvas could not be created: the app stopped".to_owned()
        ))
    );
    assert!(held.is_none());

    // The failure stays while nothing commits it again.
    assert!(!step_creation("Canvas", &mut held, &mut progress, || {
        unreachable!("a failure tries nothing until committed again")
    }));
    assert!(matches!(progress, Some(CreationProgress::Failed(_))));

    // Committed again, it tries anew.
    held = Some(commit());
    assert!(step_creation("Canvas", &mut held, &mut progress, || Ok(
        None
    )));
    assert_eq!(progress, Some(CreationProgress::Working));
    assert!(held.is_some());
}
