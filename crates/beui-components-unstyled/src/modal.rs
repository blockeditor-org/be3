use beui_view::types::{Dismiss, LayerLevel, LayerPolicy};

use beui_view::components::overlay::Overlay;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::Placement;
use beui_view::types::Pos2;
use beui_view::types::WindowId;
use beui_view::types::layer::LayerEdge;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalPlacement {
    Center,
    Fill,
    Sheet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalMotion {
    /// It shows and goes at once.
    Still,
    /// The content reports its own presence through [`ModalHandle::presence`]
    /// (as `ModalSheet`'s sheet does), and the scrim fades with it.
    Reported,
}

#[derive(Clone, Copy)]
pub struct ModalHandle {
    pub presence: Callback<f32>,
}

/// Every modal's outer part: an overlay over a scrim that fades with the
/// modal's presence.
///
/// Escape, back and a press on the scrim reach `on_dismiss` at once, but the
/// modal only leaves when its owner writes `open` false, so an owner that asks
/// first ("discard changes?") or an app that answers later keeps it in place.
/// A `Still` modal goes at once; a `Reported` one lingers while its content
/// moves away. Its content is built each time it opens and dropped once it has
/// gone; content that keeps its state while closed is a `Kept` the caller wraps
/// it in.
#[component]
pub fn Modal(
    open: Prop<bool>,
    scrim: Prop<Color>,
    #[prop(default = ModalPlacement::Center)] placement: ModalPlacement,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    motion: ModalMotion,
    #[prop(default = None)] shown_in: Prop<Option<WindowId>>,
    on_dismiss: Callback<()>,
    #[prop(children)] content: RenderFn<ModalHandle, NodeId>,
) -> NodeId {
    let presence = create_signal(match motion {
        ModalMotion::Still => 1.0_f32,
        ModalMotion::Reported => 0.0,
    });
    let handle = ModalHandle {
        presence: presence.into(),
    };
    // Built each time the modal opens, in the overlay's content, and dropped once it is gone.
    let body = move || content.call(handle);
    let placed = match placement {
        ModalPlacement::Center => Placement::Center,
        ModalPlacement::Fill => Placement::Fill,
        ModalPlacement::Sheet => Placement::Edge(LayerEdge::Bottom),
    };
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=placed
            scrim={move || scrim.get().scale_alpha(presence.get())}
            level={level}
            shown_in={shown_in}
            policy=LayerPolicy::BlocksAll
            on_dismiss={move |_: Dismiss| on_dismiss.call(())}
            content={body}
        />
    }
}
