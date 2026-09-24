use super::*;
use crate::settings::{ActivationCondition, Settings, SettingsContent};
use crate::{ChildChange, Root};
use uuid::Uuid;

#[test]
fn settings_resolve_per_client_and_follow_their_children() {
    let (kind, laptop, phone) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (shared, own, copy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let settings = edited(
        &SettingsContent::default(),
        [
            Settings::set_entry(kind, ActivationCondition::Fallback, shared),
            Settings::set_entry(kind, ActivationCondition::Client(laptop), own),
        ],
    );
    assert_eq!(settings.root().resolve(kind, laptop), Some(own));
    assert_eq!(settings.root().resolve(kind, phone), Some(shared));
    assert_eq!(settings.root().references().len(), 2);

    let replaced = edited(
        &settings,
        settings.root().child_edit(ChildChange::Replace {
            old: shared,
            new: copy,
        }),
    );
    assert_eq!(replaced.root().resolve(kind, phone), Some(copy));

    let deleted = edited(
        &replaced,
        replaced.root().child_edit(ChildChange::Delete(own)),
    );
    assert_eq!(deleted.root().resolve(kind, laptop), Some(copy));
}
