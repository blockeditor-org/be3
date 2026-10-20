use beui_core::layer::LayerPolicy;
use std::time::Duration;

use beui_macros::{component, view};

use beui_core::base::overlay::Placement;
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

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
    let shown = create_signal(false);
    let wanted = create_memo({
        let label = label.clone();
        move || !disabled.get() && !label.get().is_empty()
    });
    let open = create_memo(move || shown.get() && wanted.get());

    let dwell = create_timer(move || {
        shown.set(true);
        None
    });
    let hover = move |hovered: bool| match hovered {
        true => dwell.restart(delay),
        false => {
            dwell.stop();
            shown.set(false);
        }
    };
    let press = move |_: PointerPress| {
        dwell.stop();
        shown.set(false);
    };

    view! {
        <Interactive on_hover_change={hover} on_press={press}>
            <List @node_ref=&anchor spacing=0.0>
                {children}
                <Overlay
                    anchor=anchor
                    placement=Placement::BelowStart
                    policy=LayerPolicy::Inert
                    traps_focus=false
                    open={open}
                    content={move || content.call(TooltipHandle { label, shown: shown.read_only() })}
                />
            </List>
        </Interactive>
    }
}
