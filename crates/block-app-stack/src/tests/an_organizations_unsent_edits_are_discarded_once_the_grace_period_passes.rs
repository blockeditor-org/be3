use std::time::Duration;

use be_block::{LostAccess, OfflineChoice};
use be_client::{Clock, UnsentBlock};

use super::super::worker::{Wall, expired_unsent};
use super::*;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[test]
fn an_organizations_unsent_edits_are_discarded_once_the_grace_period_passes() {
    let clock = Clock::manual();
    let wall = Wall::new(clock.clone());
    let block = Uuid::from_u128(1);
    let unsent = [UnsentBlock {
        block,
        head: None,
        commits: Vec::new(),
        since: wall.now(),
        offerable: true,
    }];
    let deleting = OfflineChoice {
        keep: true,
        fixed: true,
        on_lost_access: LostAccess::Delete,
        grace: 7 * DAY,
    };

    assert!(expired_unsent(&unsent, true, &deleting, wall.now()).is_empty());
    clock.advance(6 * DAY);
    assert!(
        expired_unsent(&unsent, true, &deleting, wall.now()).is_empty(),
        "unsent edits wait out the grace period"
    );
    clock.advance(DAY);
    assert_eq!(
        expired_unsent(&unsent, true, &deleting, wall.now()),
        vec![block],
        "and go once it has passed"
    );
    assert!(
        expired_unsent(&unsent, false, &deleting, wall.now()).is_empty(),
        "a personal account never deletes them"
    );
    let keeping = OfflineChoice {
        on_lost_access: LostAccess::Keep,
        ..deleting
    };
    assert!(expired_unsent(&unsent, true, &keeping, wall.now()).is_empty());
}
