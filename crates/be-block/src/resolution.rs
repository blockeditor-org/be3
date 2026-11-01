use uuid::Uuid;

use crate::settings::{Follows, KindRule, Layered, layered};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SettingsSource {
    OrganizationPolicy,
    OrganizationDefault,
    Block(Uuid),
    EveryDevice,
    ThisDevice,
}

impl SettingsSource {
    pub fn is_own(self) -> bool {
        matches!(self, Self::EveryDevice | Self::ThisDevice)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolutionLayers<R> {
    pub organization_policy: Option<R>,
    pub organization_default: Option<R>,
    pub block: Option<(Uuid, R)>,
    pub every_device: Option<R>,
    pub this_device: Option<R>,
}

impl<R> Default for ResolutionLayers<R> {
    fn default() -> Self {
        Self {
            organization_policy: None,
            organization_default: None,
            block: None,
            every_device: None,
            this_device: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resolution<R> {
    layers: Vec<(SettingsSource, R)>,
    locks: usize,
}

impl<R> Default for Resolution<R> {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            locks: 0,
        }
    }
}

impl<R: Layered> Resolution<R> {
    pub fn resolve(layers: ResolutionLayers<R>) -> Self {
        let ResolutionLayers {
            organization_policy,
            organization_default,
            block,
            every_device,
            this_device,
        } = layers;
        let block = match R::FOLLOWS {
            Follows::Block => block.map(|(owner, block)| (SettingsSource::Block(owner), block)),
            Follows::Viewer => None,
        };
        let organization_default =
            organization_default.map(|layer| (SettingsSource::OrganizationDefault, layer));
        let own = [
            every_device.map(|layer| (SettingsSource::EveryDevice, layer)),
            this_device.map(|layer| (SettingsSource::ThisDevice, layer)),
        ];
        let policy = organization_policy.map(|layer| (SettingsSource::OrganizationPolicy, layer));
        let (chosen, locked): (Vec<_>, Vec<_>) = match R::RULE {
            KindRule::Default => (
                [organization_default, block]
                    .into_iter()
                    .chain(own)
                    .collect(),
                vec![policy],
            ),
            KindRule::Policy => (
                [organization_default].into_iter().chain(own).collect(),
                vec![block, policy],
            ),
        };
        let locked: Vec<(SettingsSource, R)> = locked.into_iter().flatten().collect();
        let locks = locked.len();
        let mut layers: Vec<(SettingsSource, R)> = chosen.into_iter().flatten().collect();
        layers.extend(locked);
        Self { layers, locks }
    }

    pub fn value(&self) -> R {
        layered(self.layers.iter().map(|(_, layer)| layer.clone()))
    }

    pub fn layer(&self, source: SettingsSource) -> Option<&R> {
        self.layers
            .iter()
            .find(|(held, _)| *held == source)
            .map(|(_, layer)| layer)
    }

    pub fn sources(&self) -> impl Iterator<Item = SettingsSource> + '_ {
        self.layers.iter().map(|(source, _)| *source)
    }

    pub fn source_of<V>(&self, get: impl Fn(&R) -> Option<V>) -> Option<SettingsSource> {
        self.layers
            .iter()
            .rev()
            .find(|(_, layer)| get(layer).is_some())
            .map(|(source, _)| *source)
    }

    pub fn locked_by<V>(&self, get: impl Fn(&R) -> Option<V>) -> Option<SettingsSource> {
        let first = self.layers.len() - self.locks;
        self.layers[first..]
            .iter()
            .rev()
            .find(|(_, layer)| get(layer).is_some())
            .map(|(source, _)| *source)
    }

    pub fn below_own(&self) -> R {
        let first = self.layers.len() - self.locks;
        layered(
            self.layers[..first]
                .iter()
                .filter(|(source, _)| !source.is_own())
                .map(|(_, layer)| layer.clone()),
        )
    }
}
