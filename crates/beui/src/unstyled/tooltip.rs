use std::time::Duration;

use beui_macros::{component, view};

use crate::base::overlay::{Overlay, OverlayMode, Placement};
use crate::input::PointerPress;
use crate::node::NodeId;
use crate::reactive::{
    Child, ClickCatcher, List, NodeRef, Prop, ReadSignal, Render, clone, create_memo,
    create_signal, create_timer,
};

pub const TOOLTIP_DELAY: Duration = Duration::from_millis(450);

pub struct TooltipHandle {
    pub label: Prop<String>,
    pub shown: ReadSignal<bool>,
}

#[component]
pub fn Tooltip(
    label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TOOLTIP_DELAY)] delay: Duration,
    children: Child,
    content: Render<TooltipHandle>,
) -> NodeId {
    let anchor = NodeRef::new();
    let (shown, set_shown) = create_signal(false);
    let wanted = create_memo(clone!(label disabled -> move || {
        !disabled.get() && !label.get().is_empty()
    }));
    let open = create_memo(clone!(shown wanted -> move || shown.get() && wanted.get()));

    let dwell = create_timer(clone!(set_shown -> move || {
        set_shown.set(true);
        None
    }));
    let hover = clone!(dwell set_shown -> move |hovered: bool| {
        match hovered {
            true => dwell.restart(delay),
            false => {
                dwell.stop();
                set_shown.set(false);
            }
        }
    });
    let press = clone!(dwell set_shown -> move |_: PointerPress| {
        dwell.stop();
        set_shown.set(false);
    });
    let bubble = content.call(TooltipHandle { label, shown });

    view! {
        <ClickCatcher on_hover_change={hover} on_press={press}>
            <List @node_ref=&anchor spacing=0.0>
                {children}
                <Overlay
                    anchor=&anchor
                    placement=Placement::BelowStart
                    mode=OverlayMode::Passive
                    traps_focus=false
                    open={open}
                >
                    {bubble}
                </Overlay>
            </List>
        </ClickCatcher>
    }
}
