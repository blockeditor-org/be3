use beui_view::types::{Dismiss, LayerLevel, LayerPolicy};

use beui_view::components::overlay::Overlay;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::Placement;
use beui_view::types::Pos2;
use beui_view::types::WindowId;

/// A plain overlay over its whole window, on a scrim: what is not a window
/// (decisions/a_dialog_is_a_window_of_its_owner_not_of_the_dock.md) and must
/// never move or be covered, as be-host's lock screen, and `Fullscreen`.
///
/// Escape, back and a press on the scrim reach `on_dismiss` at once, but the
/// modal only leaves when its owner writes `open` false, so an owner that asks
/// first ("discard changes?") keeps it in place; then it goes at once. Its
/// content is built each time it opens and dropped once it has gone; content
/// that keeps its state while closed is a `Kept` the caller wraps it in.
#[component]
pub fn Modal(
    open: Prop<bool>,
    scrim: Prop<Color>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = None)] shown_in: Prop<Option<WindowId>>,
    on_dismiss: Callback<()>,
    #[prop(children)] content: BuildFn<NodeId>,
) -> NodeId {
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Fill
            scrim={scrim}
            level={level}
            shown_in={shown_in}
            policy=LayerPolicy::BlocksAll
            on_dismiss={move |_: Dismiss| on_dismiss.call(())}
            content={move || content.call()}
        />
    }
}
