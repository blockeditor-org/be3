use super::*;
use crate::{
    LostAccess, OfflinePolicy, OfflineSettings, OfflineSettingsContent, Resolution,
    ResolutionLayers,
};

fn layer(edits: &[Edit]) -> OfflineSettings {
    let mut content = OfflineSettingsContent::default();
    for edit in edits {
        content.apply(edit);
    }
    content.root()
}

#[test]
fn offline_first_is_chosen_per_workspace_within_the_organizations_policy() {
    let [kept, dropped, unset] = std::array::from_fn(|_| Uuid::new_v4());
    let own = layer(&[
        OfflineSettings::keep_workspace(kept, Some(true)),
        OfflineSettings::keep_workspace(dropped, Some(false)),
        OfflineSettings::set_on_lost_access(Some(LostAccess::Delete)),
    ]);
    let resolve = |policy: Option<OfflineSettings>| {
        Resolution::resolve(ResolutionLayers {
            organization_policy: policy,
            this_device: Some(own.clone()),
            ..ResolutionLayers::default()
        })
    };

    let free = resolve(None);
    assert!(OfflineSettings::choice(&free, kept, false).keep);
    assert!(!OfflineSettings::choice(&free, dropped, false).keep);
    assert!(
        OfflineSettings::choice(&free, unset, false).keep,
        "a workspace nobody chose for is kept"
    );
    assert_eq!(
        OfflineSettings::choice(&free, kept, false).on_lost_access,
        LostAccess::Keep,
        "only the organization decides what happens to what a device kept"
    );

    let required = resolve(Some(layer(&[
        OfflineSettings::set_policy(Some(OfflinePolicy::Required)),
        OfflineSettings::set_on_lost_access(Some(LostAccess::Delete)),
        OfflineSettings::set_grace_days(Some(2)),
    ])));
    let choice = OfflineSettings::choice(&required, dropped, false);
    assert!(choice.keep && choice.fixed);
    assert_eq!(choice.on_lost_access, LostAccess::Delete);
    assert_eq!(
        choice.grace,
        std::time::Duration::from_secs(2 * 24 * 60 * 60)
    );
    assert_eq!(
        OfflineSettings::choice(&required, dropped, true).on_lost_access,
        LostAccess::Keep,
        "a personal account never deletes anything by itself"
    );

    let blocked = resolve(Some(layer(&[OfflineSettings::set_policy(Some(
        OfflinePolicy::Blocked,
    ))])));
    let choice = OfflineSettings::choice(&blocked, kept, false);
    assert!(!choice.keep && choice.fixed);
}
