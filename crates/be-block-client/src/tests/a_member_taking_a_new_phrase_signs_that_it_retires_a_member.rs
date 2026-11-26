use super::*;

use be_block_keys::{Administrator, Entry, LogKind, Record, RecoveryPhrase, Step};
use be_block_protocol::{OrganizationPath, RecoveryKey, SignedLog};

/// The device of an account a proven path shows a member of an organization at the log's head
/// signs that its retirement retires a member (`Retirement::administrator` false), which is what
/// lets a path hide it: said of a log only where `contacts::not_administered` proved it.
#[test]
fn a_member_taking_a_new_phrase_signs_that_it_retires_a_member() {
    let creator = RecoveryPhrase::generate().secret();
    let deputy = RecoveryPhrase::generate().secret();
    let fresh = RecoveryPhrase::generate().secret();
    let (creator_id, deputy_id) = (Uuid::new_v4(), Uuid::new_v4());
    let organization = Uuid::new_v4();
    let root = Entry::root(
        &creator,
        creator_id.into_bytes(),
        LogKind::Organization,
        organization.into_bytes(),
    );
    let mut steps: Vec<Step> = vec![root.clone().into()];
    let mut next = |administrator| {
        let entry = be_block_keys::check_log(
            LogKind::Organization,
            organization.into_bytes(),
            root.hash(),
            &steps,
            &|_| None,
        )
        .unwrap()
        .next(
            &creator,
            creator_id.into_bytes(),
            organization.into_bytes(),
            Record::Administrator(Administrator {
                account: deputy_id.into_bytes(),
                identity: deputy.identity(),
                administrator,
            }),
        )
        .unwrap();
        steps.push(entry.clone().into());
        entry.hash()
    };
    let promoted = next(true);
    let demoted = next(false);
    let signed = |kind, head: [u8; 32]| SignedLog {
        organization,
        kind,
        log: organization,
        head: head.to_vec(),
    };
    let answer = RecoveryKey {
        public: None,
        sealed: Vec::new(),
        logs: vec![signed(be_block_protocol::LogKind::Organization, demoted)],
        organization_logs: vec![OrganizationPath {
            organization,
            steps: steps.iter().map(Step::encode).collect(),
        }],
    };
    let not_administered = crate::contacts::not_administered(deputy_id, &answer).unwrap();
    let moved = crate::contacts::new_identity(
        deputy_id,
        &deputy,
        &[],
        &fresh,
        Vec::new(),
        &std::collections::HashMap::new(),
        answer.logs.clone(),
        &not_administered,
    )
    .unwrap();
    let says: Vec<bool> = moved
        .retirements
        .iter()
        .map(
            |retirement| match Entry::decode(&retirement.entry).unwrap().record {
                Record::Retirement(retired) => retired.administrator,
                other => panic!("a retirement: {other:?}"),
            },
        )
        .collect();
    let _ = promoted;
    assert_eq!(
        not_administered,
        [(organization, demoted)].into_iter().collect(),
        "the path proves the deputy a member at the head after the demotion"
    );
    assert_eq!(
        says,
        [false],
        "a member's retirement says it retires a member"
    );
}
