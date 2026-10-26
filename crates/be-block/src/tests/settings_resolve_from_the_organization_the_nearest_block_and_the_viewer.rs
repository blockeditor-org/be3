use be_model::Model;

use super::*;
use crate::settings::{Follows, KindRule, Layered};
use crate::{Resolution, ResolutionLayers, Root, SettingsSource};

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Shown {
    color: Option<u8>,
    size: Option<u8>,
}

impl Root for Shown {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x5e77);
}

impl Layered for Shown {
    const NAME: &'static str = "Shown";
    const FOLLOWS: Follows = Follows::Block;

    fn over(&self, lower: &Self) -> Self {
        Self {
            color: self.color.or(lower.color),
            size: self.size.or(lower.size),
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Kept {
    offline: Option<bool>,
}

impl Root for Kept {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x5e78);
}

impl Layered for Kept {
    const NAME: &'static str = "Kept";
    const FOLLOWS: Follows = Follows::Block;
    const RULE: KindRule = KindRule::Policy;

    fn over(&self, lower: &Self) -> Self {
        Self {
            offline: self.offline.or(lower.offline),
        }
    }
}

fn shown(color: Option<u8>, size: Option<u8>) -> Shown {
    Shown { color, size }
}

#[test]
fn settings_resolve_from_the_organization_the_nearest_block_and_the_viewer() {
    let folder = Uuid::from_u128(0xf0);
    let resolved = Resolution::resolve(ResolutionLayers {
        organization_policy: Some(shown(None, Some(9))),
        organization_default: Some(shown(Some(1), Some(1))),
        block: Some((folder, shown(Some(2), None))),
        every_device: Some(shown(None, Some(3))),
        this_device: None,
    });
    assert_eq!(resolved.value(), shown(Some(2), Some(9)));
    assert_eq!(
        resolved.source_of(|layer| layer.color),
        Some(SettingsSource::Block(folder))
    );
    assert_eq!(
        resolved.source_of(|layer| layer.size),
        Some(SettingsSource::OrganizationPolicy)
    );
    assert_eq!(
        resolved.locked_by(|layer| layer.size),
        Some(SettingsSource::OrganizationPolicy)
    );
    assert_eq!(resolved.locked_by(|layer| layer.color), None);
    assert_eq!(resolved.below_own(), shown(Some(2), Some(1)));

    let mine = Resolution::resolve(ResolutionLayers {
        block: Some((folder, shown(Some(2), None))),
        every_device: Some(shown(Some(4), None)),
        this_device: Some(shown(Some(5), None)),
        ..ResolutionLayers::default()
    });
    assert_eq!(mine.value(), shown(Some(5), None));
    assert_eq!(
        mine.source_of(|layer| layer.color),
        Some(SettingsSource::ThisDevice)
    );

    let policy = Resolution::resolve(ResolutionLayers {
        block: Some((
            folder,
            Kept {
                offline: Some(false),
            },
        )),
        this_device: Some(Kept {
            offline: Some(true),
        }),
        ..ResolutionLayers::default()
    });
    assert_eq!(
        policy.value(),
        Kept {
            offline: Some(false)
        }
    );
    assert_eq!(
        policy.locked_by(|layer| layer.offline),
        Some(SettingsSource::Block(folder))
    );

    let viewer = Resolution::resolve(ResolutionLayers {
        block: Some((folder, crate::DisplaySettings::default())),
        ..ResolutionLayers::default()
    });
    assert_eq!(viewer.sources().count(), 0);
}
