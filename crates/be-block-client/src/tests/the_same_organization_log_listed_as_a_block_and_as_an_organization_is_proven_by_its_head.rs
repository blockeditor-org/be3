use super::*;

use be_block_keys::{Administrator, Entry, LogKind, Record, RecoveryPhrase, Step};
use be_block_protocol::{OrganizationPath, RecoveryKey, SignedLog};

/// What a proven path shows is keyed by the log and the head (`contacts::not_administered`): the
/// same organization log listed twice at the same head, once as a block's and once as the
/// organization's, signs that it retires a member in both, and a listing of it at a head no
/// path proved signs that it may administer.
#[test]
fn the_same_organization_log_listed_as_a_block_and_as_an_organization_is_proven_by_its_head() {
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
        logs: vec![
            signed(be_block_protocol::LogKind::Block, demoted),
            signed(be_block_protocol::LogKind::Organization, demoted),
            signed(be_block_protocol::LogKind::Block, promoted),
        ],
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
        .map(|retirement| match Entry::decode(&retirement.entry).unwrap().record {
            Record::Retirement(retired) => retired.administrator,
            other => panic!("a retirement: {other:?}"),
        })
        .collect();
    assert_eq!(
        not_administered,
        [(organization, demoted)].into_iter().collect(),
        "only the organization's listing is proven, at its head"
    );
    assert_eq!(
        says,
        [false, false, true],
        "the same head signs false in both listings, and the head no path proved signs true"
    );
}
