use super::*;

use be_block_keys::{Administrator, Entry, LogKind, Record, RecoveryPhrase, Step};
use be_block_protocol::{RecoveryKey, SignedLog};

/// A server that labels an organization's log a block's (`SignedLog::kind`) serves no path of it
/// to prove anything by, so the administrator taking a new phrase signs there that their
/// retirement may retire an administrator (`Retirement::administrator`): a path that reduces it
/// is still refused authority, so their stolen old key grants nothing on the organization's.
/// Had the device signed that it retires a member, a path could hide it.
#[test]
fn a_new_phrase_in_an_organization_log_the_server_labels_a_blocks_says_it_may_administer() {
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
    let check = |steps: &[Step]| {
        be_block_keys::check_log(
            LogKind::Organization,
            organization.into_bytes(),
            root.hash(),
            steps,
            &|_| None,
        )
    };
    let mut steps: Vec<Step> = vec![root.clone().into()];
    let promotion = check(&steps)
        .unwrap()
        .next(
            &creator,
            creator_id.into_bytes(),
            organization.into_bytes(),
            Record::Administrator(Administrator {
                account: deputy_id.into_bytes(),
                identity: deputy.identity(),
                administrator: true,
            }),
        )
        .unwrap();
    steps.push(promotion.clone().into());

    let answer = RecoveryKey {
        public: None,
        sealed: Vec::new(),
        logs: vec![SignedLog {
            organization,
            kind: be_block_protocol::LogKind::Block,
            log: organization,
            head: promotion.hash().to_vec(),
        }],
        organization_logs: Vec::new(),
    };
    let not_administered = crate::contacts::not_administered(deputy_id, &answer).unwrap();
    assert!(not_administered.is_empty());
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
    let [retirement] = &moved.retirements[..] else {
        panic!("one retirement for the one log: {:?}", moved.retirements);
    };
    let retirement = Entry::decode(&retirement.entry).unwrap();
    assert!(
        matches!(&retirement.record, Record::Retirement(retired) if retired.administrator),
        "a retirement in a log no path proves says it may administer: {retirement:?}"
    );

    let whole = [steps.clone(), vec![retirement.clone().into()]].concat();
    assert!(
        check(&whole).is_ok(),
        "the organization's log takes the retirement"
    );
    let hidden = [steps.clone(), vec![Step::Reduced(retirement.reduced())]].concat();
    assert!(
        check(&hidden).unwrap().hides_retirement,
        "a path that reduces it hides an administrator's retirement"
    );
}
