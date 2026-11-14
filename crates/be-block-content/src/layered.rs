//! What makes a type a settings kind (guides/settings.md): one block per kind in each layer,
//! combined by the host's `Resolution` from the layers' `over`.

use be_block_model::Map;

use crate::Root;

/// A settings kind: one block per kind in each layer, combined by
/// `Resolution`. `over` combines an upper layer with a lower one.
pub trait Layered: Root + Clone + Default {
    const NAME: &'static str;
    const FOLLOWS: Follows = Follows::Viewer;
    const RULE: KindRule = KindRule::Default;
    const VISIBILITY: KindVisibility = KindVisibility::Members;

    fn over(&self, lower: &Self) -> Self;
}

/// Whether the block layer applies. See
/// decisions/settings_follow_the_block_or_the_viewer.md.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Follows {
    /// The data's settings, resolved from the nearest block above it that has
    /// this kind.
    Block,
    /// The viewer's own preference: the block layer is skipped.
    Viewer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KindRule {
    /// Later layers win.
    Default,
    /// The block layer's set fields are locked against the user's own layers.
    Policy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KindVisibility {
    /// The client grants the kind View to `ANYONE_INSIDE` when it makes it, so
    /// anyone viewing any block inside the owner reads it (a block shared alone
    /// still resolves its row ids and shows its tags).
    Inside,
    #[default]
    Members,
    /// Only the account that made the kind's block reads it, on any of its
    /// devices: the block is its own scope root, created `owner-only`, and its
    /// key is sealed to that account alone, never to the organization's escrow
    /// key (decisions/owner_only_settings_are_never_escrowed.md).
    Owner,
}

impl KindVisibility {
    pub const fn owner_only(self) -> bool {
        matches!(self, Self::Owner)
    }
}

pub fn union<K: Ord + Clone, V: Clone>(upper: &Map<K, V>, lower: &Map<K, V>) -> Map<K, V> {
    lower
        .iter()
        .chain(upper.iter())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub fn layered<R: Layered>(layers: impl IntoIterator<Item = R>) -> R {
    layers
        .into_iter()
        .fold(R::default(), |lower, upper| upper.over(&lower))
}
