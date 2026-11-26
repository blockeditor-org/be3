use super::*;

use be_block_keys::{Administrator, Entry, LogKind, Member, Record, RecoveryPhrase, Step};
use be_block_protocol::{OrganizationPath, RecoveryKey, SignedLog};

/// The device taking a new phrase signs that it retires a member in an organization's log only
/// when that log's proof path, checked from its root, proves it (`contacts::not_administered`):
/// a listed log the answer carries no path for, a path that does not reach the head named, one
/// that reduces an administrator record or hides a lowering, or a log that is not the
/// organization's own, is forged.
#[test]
fn an_organization_log_served_for_a_new_phrase_is_refused_unless_its_path_proves_it() {
    let creator = RecoveryPhrase::generate().secret();
    let deputy = RecoveryPhrase::generate().secret();
    let (creator_id, deputy_id) = (Uuid::new_v4(), Uuid::new_v4());
    let organization = Uuid::new_v4();
    let root = Entry::root(
        &creator,
        creator_id.into_bytes(),
        LogKind::Organization,
        organization.into_bytes(),
    );
    let next = |steps: &[Step], record| {
        be_block_keys::check_log(
            LogKind::Organization,
            organization.into_bytes(),
            root.hash(),
            steps,
            &|_| None,
        )
        .unwrap()
        .next(
            &creator,
            creator_id.into_bytes(),
            organization.into_bytes(),
            record,
        )
        .unwrap()
    };
    let administrator = |administrator| {
        Record::Administrator(Administrator {
            account: deputy_id.into_bytes(),
            identity: deputy.identity(),
            administrator,
        })
    };
    let mut steps: Vec<Step> = vec![root.clone().into()];
    let promotion = next(&steps, administrator(true));
    steps.push(promotion.clone().into());
    let promoted = promotion.hash();
    // The deputy's removal, in place of the demotion: it lowers them, and is no administrator
    // record.
    let removal = next(
        &steps,
        Record::Member(Member {
            account: deputy_id.into_bytes(),
            identity: None,
            member: false,
            agent: false,
            invitation: None,
        }),
    );
    assert!(removal.lowers);
    let hidden_removal = [steps.clone(), vec![Step::Reduced(removal.reduced())]].concat();
    let demotion = next(&steps, administrator(false));
    steps.push(demotion.clone().into());
    let demoted = demotion.hash();

    let answer = |log: Uuid, head: [u8; 32], path: Option<&[Step]>| RecoveryKey {
        public: None,
        sealed: Vec::new(),
        logs: vec![SignedLog {
            organization,
            kind: be_block_protocol::LogKind::Organization,
            log,
            head: head.to_vec(),
        }],
        organization_logs: path
            .map(|path| OrganizationPath {
                organization,
                steps: path.iter().map(Step::encode).collect(),
            })
            .into_iter()
            .collect(),
    };
    let members = |answer: &RecoveryKey| crate::contacts::not_administered(deputy_id, answer);
    let forged = |answer: &RecoveryKey| {
        matches!(members(answer), Err(ClientError::Forged(forged)) if forged == organization)
    };

    // An honest path proves the deputy a member at the head after the demotion, and an
    // administrator at the head after the promotion, where it says nothing.
    assert_eq!(
        members(&answer(organization, demoted, Some(&steps))).unwrap(),
        [(organization, demoted)].into_iter().collect()
    );
    assert!(
        members(&answer(organization, promoted, Some(&steps)))
            .unwrap()
            .is_empty()
    );

    assert!(
        forged(&answer(organization, demoted, None)),
        "an organization's log listed with no path is forged"
    );
    assert!(
        forged(&answer(organization, [5; 32], Some(&steps))),
        "a path that does not hold the head named is forged"
    );
    let reduced_promotion = vec![
        root.clone().into(),
        Step::Reduced(promotion.reduced()),
        demotion.clone().into(),
    ];
    assert!(
        forged(&answer(organization, demoted, Some(&reduced_promotion))),
        "a path that reduces an administrator record is forged"
    );
    assert!(
        forged(&answer(organization, removal.hash(), Some(&hidden_removal))),
        "a path that hides a lowering is forged"
    );
    assert!(
        forged(&answer(Uuid::new_v4(), demoted, Some(&steps))),
        "an organization's log that is not the organization's own is forged"
    );
}
