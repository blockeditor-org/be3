use beui_core::layer::{Dismiss, LayerLevel};
use beui_macros::{component, view};

use super::back_slide::BackSlide;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_core::screens::bounds;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalPlacement {
    Center,
    Fill,
    AllScreens,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalMotion {
    Slides,
    Still,
    Reported,
}

#[derive(Clone, Copy)]
pub struct ModalHandle {
    pub presence: Callback<f32>,
}

#[component]
pub fn Modal(
    open: Prop<bool>,
    scrim: Prop<Color32>,
    #[prop(default = ModalPlacement::Center)] placement: ModalPlacement,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = ModalMotion::Slides)] motion: ModalMotion,
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render<ModalHandle>,
) -> NodeId {
    let presence = create_signal(match motion {
        ModalMotion::Slides | ModalMotion::Still => 1.0_f32,
        ModalMotion::Reported => 0.0,
    });
    let faded = create_memo(move || scrim.get().scale_alpha(presence.get()));
    let handle = ModalHandle {
        presence: presence.into(),
    };
    let body = Render::new(move |()| {
        let content = content.call(handle);
        match motion {
            ModalMotion::Slides => view! {
                <BackSlide enters=false on_back={on_dismiss} on_presence={presence}>
                    {content}
                </BackSlide>
            },
            ModalMotion::Still | ModalMotion::Reported => content,
        }
    })
    .once();
    let screens = use_screens();
    let anchor = create_memo(move || match placement {
        ModalPlacement::AllScreens => {
            OverlayAnchor::Point(screens.with(|screens| bounds(screens)).min)
        }
        ModalPlacement::Center | ModalPlacement::Fill => OverlayAnchor::Point(Pos2::ZERO),
    });
    let placed = match placement {
        ModalPlacement::Center => Placement::Center,
        ModalPlacement::Fill => Placement::Fill,
        ModalPlacement::AllScreens => Placement::At,
    };
    view! {
        <Overlay
            anchor={anchor}
            open={open}
            keep_alive=true
            placement=placed
            scrim={faded}
            level={level}
            on_dismiss={move |_: Dismiss| on_dismiss.call()}
            content={move || body.call(())}
        />
    }
}
