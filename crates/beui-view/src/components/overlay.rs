use crate::reactive::Callback;
use crate::reactive::NodeId;
use crate::reactive::NodeRef;
use crate::reactive::OverlayAnchor;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
use crate::reactive::{Get, Memo, Set, Signal, create_memo, create_signal, use_context};
use crate::types::Color;
use crate::types::Dismiss;
use crate::types::LayerLevel;
use crate::types::LayerPolicy;
use crate::types::Placement;
use crate::types::WindowId;
use crate::types::WindowSpec;
use beui_macros::component;

#[component]
pub fn Overlay(
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = Color::TRANSPARENT)] scrim: Prop<Color>,
    #[prop(default = 0.0)] scrim_radius: Prop<f32>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = None)] shown_in: Prop<Option<WindowId>>,
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    #[prop(default = 0.0)] beak: Prop<f32>,
    #[prop(default = None)] window: Prop<Option<WindowSpec>>,
    open: Prop<bool>,
    #[prop(default = false)] keep_alive: bool,
    #[prop(default = true)] dismissable: Prop<bool>,
    on_dismiss: Callback<Dismiss>,
    #[prop(children)] content: RenderFn<()>,
) -> NodeId {
    let presence = OverlayPresence::new(create_memo(move || open.get()));
    crate::core::overlay(
        anchor,
        placement,
        scrim,
        scrim_radius,
        policy,
        level,
        traps_focus,
        shown_in,
        trigger,
        pointer,
        beak,
        window,
        keep_alive,
        dismissable,
        on_dismiss,
        presence,
        content,
    )
}

#[derive(Clone, Copy)]
pub struct OverlayPresence {
    open: Memo<bool>,
    lingering: Signal<Vec<Memo<bool>>>,
}

impl OverlayPresence {
    pub(crate) fn new(open: Memo<bool>) -> Self {
        Self {
            open,
            lingering: create_signal(Vec::new()),
        }
    }

    pub(crate) fn lingering(self) -> Signal<Vec<Memo<bool>>> {
        self.lingering
    }

    pub(crate) fn shown(self) -> Memo<bool> {
        create_memo(move || {
            self.open.get()
                || self
                    .lingering
                    .with(|lingering| lingering.iter().any(Get::get))
        })
    }

    pub fn open(self) -> Memo<bool> {
        self.open
    }

    pub fn linger_while(self, showing: Memo<bool>) {
        self.lingering.update(|lingering| lingering.push(showing));
    }
}

pub fn use_overlay_presence() -> Option<OverlayPresence> {
    use_context::<OverlayPresence>()
}
