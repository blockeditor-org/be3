use beui_macros::{component, view};
use beui_view::types::{Dismiss, LayerLevel, LayerPolicy};

use super::back_slide::BackSlide;
use beui_view::components::overlay::{Overlay, use_overlay_presence};
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
    /// The content sits in a [`BackSlide`], which carries it away on back.
    Slides,
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
/// While leaving it lingers, inert.
#[component]
pub fn Modal(
    open: Prop<bool>,
    scrim: Prop<Color>,
    #[prop(default = ModalPlacement::Center)] placement: ModalPlacement,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = ModalMotion::Slides)] motion: ModalMotion,
    #[prop(default = None)] shown_in: Prop<Option<WindowId>>,
    on_dismiss: Callback<()>,
    #[prop(children)] content: Render<ModalHandle>,
) -> NodeId {
    let presence = create_signal(match motion {
        ModalMotion::Slides | ModalMotion::Still => 1.0_f32,
        ModalMotion::Reported => 0.0,
    });
    let faded = create_memo(move || scrim.get().scale_alpha(presence.get()));
    let asked = create_signal(false);
    create_effect(move || {
        if open.get() {
            untrack(|| asked.set(false));
        }
    });
    let stays = create_memo(move || open.get() || !asked.get());
    let handle = ModalHandle {
        presence: presence.into(),
    };
    let body = Render::new(move |()| {
        let content = content.call(handle);
        match motion {
            ModalMotion::Slides => {
                if let Some(overlay) = use_overlay_presence() {
                    overlay.linger_while(create_memo(move || asked.get() && !open.get()));
                }
                view! {
                    <BackSlide
                        enters=false
                        open={stays}
                        on_back={move || {
                            asked.set(true);
                            on_dismiss.call(());
                        }}
                        on_gone={move || asked.set(false)}
                        on_presence={presence}
                    >
                        {content}
                    </BackSlide>
                }
            }
            ModalMotion::Still | ModalMotion::Reported => content,
        }
    })
    .once();
    let placed = match placement {
        ModalPlacement::Center => Placement::Center,
        ModalPlacement::Fill => Placement::Fill,
        ModalPlacement::Sheet => Placement::Edge(LayerEdge::Bottom),
    };
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            keep_alive=true
            placement=placed
            scrim={faded}
            level={level}
            shown_in={shown_in}
            policy={create_memo(move || match asked.get() && !open.get() {
                true => LayerPolicy::Inert,
                false => LayerPolicy::BlocksAll,
            })}
            on_dismiss={move |_: Dismiss| on_dismiss.call(())}
            content={move || body.call(())}
        />
    }
}
