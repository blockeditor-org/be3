use super::*;

use be_block_keys::{
    Administrator, CarriedKey, Entry, Heads, LogKind, Party, Record, RecoveryPhrase, Step,
};
use be_block_protocol::{KeyKind, KeyRow, KeySet, WrappedBy, WrappedKey};
use be_block_store::Keyring;

/// An honest proof path of an organization's log keeps every administrator record whole
/// (plans/signed_access.md, 3e), so one that reduces an entry that lowered anyone hides a
/// demotion. A reader served such a path names no administrator by it: a key the demoted
/// administrator seals it as one is not taken, though the same seal is taken by a path that
/// shows them administering still, at the head before the demotion.
#[test]
fn a_demotion_hidden_from_an_organization_path_names_no_administrator() {
    let creator = RecoveryPhrase::generate().secret();
    let deputy = RecoveryPhrase::generate().secret();
    let reader = RecoveryPhrase::generate().secret();
    let (creator_id, deputy_id, reader_id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
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
            &|_, _| None,
        )
        .unwrap()
    };
    let make = |steps: &[Step], administrator| {
        check(steps)
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
            .unwrap()
    };
    let mut steps: Vec<Step> = vec![root.clone().into()];
    steps.push(make(&steps, true).into());
    let promoted = steps.last().unwrap().hash();
    let demotion = make(&steps, false);
    assert!(demotion.lowers);
    let hidden = [steps.clone(), vec![Step::Reduced(demotion.reduced())]].concat();
    let checked = check(&hidden);
    assert!(checked.hides_lowering);
    assert!(
        checked.administrators.is_empty()
            && checked
                .level_as_of(deputy_id.into_bytes(), &promoted)
                .is_none(),
        "a path hiding a demotion names no administrator, as of no entry"
    );

    // The deputy seals the reader a data key, as it would the settings' to an administrator.
    let seal = |key: Uuid| {
        let carried = CarriedKey {
            key: [3; 32],
            block: Uuid::new_v4().into_bytes(),
            root: None,
            heads: Heads {
                block: None,
                organization: Some(promoted),
            },
        };
        let block = Uuid::from_bytes(carried.block);
        KeySet {
            keys: vec![KeyRow {
                id: key,
                block,
                kind: KeyKind::Data,
                epoch: 1,
            }],
            wrapped: vec![WrappedKey {
                key,
                wrapped_by: WrappedBy::Account(reader_id),
                sender: deputy_id,
                sealed: deputy.seal_for(
                    deputy_id.into_bytes(),
                    &Party {
                        account: reader_id.into_bytes(),
                        public: reader.public(),
                    },
                    key.as_bytes(),
                    &carried.encode(),
                ),
            }],
        }
    };
    let book = |log: &be_block_keys::Checked| {
        let book = crate::KeyBook::new(reader_id, organization, reader.clone(), Keyring::new());
        book.set_contacts([(deputy_id, deputy.public())]);
        book.set_organization(Some(log));
        book
    };

    let shown = book(&check(&steps));
    let key = Uuid::new_v4();
    shown.absorb(seal(key));
    assert!(
        shown.ring().get(key).is_some(),
        "a seal from an administrator the log names is not taken"
    );

    let fooled = book(&checked);
    let key = Uuid::new_v4();
    fooled.absorb(seal(key));
    assert!(
        fooled.ring().get(key).is_none(),
        "a seal from an administrator whose demotion the path hides is taken"
    );
}
