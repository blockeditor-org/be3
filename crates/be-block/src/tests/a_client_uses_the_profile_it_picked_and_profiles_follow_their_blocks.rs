use super::*;
use crate::settings::{Settings, SettingsContent};
use crate::{ChildChange, Root};
use uuid::Uuid;

#[test]
fn a_client_uses_the_profile_it_picked_and_profiles_follow_their_blocks() {
    let (laptop, phone) = (Uuid::new_v4(), Uuid::new_v4());
    let (desk, travel, copy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let settings = edited(
        &SettingsContent::default(),
        [
            Settings::add_profile(laptop, false, desk),
            Settings::add_profile(phone, false, travel),
        ],
    );
    assert_eq!(settings.root().profile(laptop, false), Some(desk));
    assert_eq!(settings.root().profile(phone, false), Some(travel));
    assert_eq!(settings.root().profile(Uuid::new_v4(), false), None);
    assert_eq!(settings.root().references().len(), 2);

    let switched = edited(&settings, [Settings::use_profile(phone, false, desk)]);
    assert_eq!(switched.root().profile(phone, false), Some(desk));
    assert_eq!(
        switched.root().profiles().len(),
        2,
        "the other profile stays"
    );

    let replaced = edited(
        &switched,
        switched.root().child_edit(ChildChange::Replace {
            old: desk,
            new: copy,
        }),
    );
    assert_eq!(replaced.root().profile(laptop, false), Some(copy));
    assert!(!replaced.root().profiles().contains(&desk));

    let deleted = edited(
        &replaced,
        replaced.root().child_edit(ChildChange::Delete(travel)),
    );
    assert_eq!(deleted.root().profiles(), [copy]);
}
