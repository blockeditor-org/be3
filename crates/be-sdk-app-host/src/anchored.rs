//! What a press closes of the views an editor anchored outside its groups (menus, popovers,
//! sheets), as every host decides it: be-host's compositor and a root guest alike.

use be_lib_geometry::{Pos2, Rect};
use be_sdk_api::ViewId;

/// A view an editor anchored, as a host shows it, in its host's coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Hung {
    pub id: ViewId,
    /// The anchored view it hangs from, if it hangs from one rather than from the view that
    /// owns the chain.
    pub parent: Option<ViewId>,
    /// Where its overlays take input (`frame-report.floating`).
    pub floating: Vec<Rect>,
    pub dismiss_on_outside_press: bool,
    /// The control that opens and closes it (`anchored-placement.trigger`).
    pub trigger: Option<Rect>,
    pub blocks_owner: bool,
}

impl Hung {
    /// Whether a press at `pos` lands on it.
    pub fn takes(&self, pos: Pos2) -> bool {
        self.floating.iter().any(|rect| rect.contains(pos))
    }
}

/// The views of `layers`, one owner's in the order they opened, that a press at `pos` closes:
/// each that asks to be closed by a press outside it (`dismiss-on-outside-press`), with the
/// views that hang from it, unless the press lands on it, on a view hanging from it, or on its
/// trigger (the trigger's own click closes it), a blocking view's trigger only when the press
/// reaches the trigger through another view.
pub fn dismissed_by_press(layers: &[Hung], pos: Pos2) -> Vec<ViewId> {
    let below = |root: ViewId| {
        let mut chain = vec![root];
        for layer in layers {
            if let Some(parent) = layer.parent
                && chain.contains(&parent)
                && !chain.contains(&layer.id)
            {
                chain.push(layer.id);
            }
        }
        chain
    };
    let forwarded = layers.iter().any(|held| held.takes(pos));
    let mut dismissed = Vec::new();
    for layer in layers {
        if !layer.dismiss_on_outside_press || dismissed.contains(&layer.id) {
            continue;
        }
        let chain = below(layer.id);
        let inside = layers
            .iter()
            .filter(|held| chain.contains(&held.id))
            .any(|held| {
                held.takes(pos)
                    || held
                        .trigger
                        .is_some_and(|rect| rect.contains(pos) && (forwarded || !held.blocks_owner))
            });
        if !inside {
            for id in chain {
                if !dismissed.contains(&id) {
                    dismissed.push(id);
                }
            }
        }
    }
    dismissed
}

#[cfg(test)]
mod tests;
