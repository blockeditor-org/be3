use std::time::Duration;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::theme::use_theme;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Direction, Frame, ItemSize, List, Prop, Spacer, clone, component_accessibility,
    component_placed, create_effect, create_memo, create_signal, create_timer, now,
};

const HEIGHT: f32 = 4.0;
const RADIUS: u8 = 2;
const SEGMENT: f32 = 0.35;
const PERIOD: f32 = 1.2;
const FRAME: Duration = Duration::from_millis(16);

#[component]
pub fn Spinner(
    #[prop(default = 40.0)] width: Prop<f32>,
    #[prop(default = String::new())] label: Prop<String>,
) -> NodeId {
    let started = now();
    let (phase, set_phase) = create_signal(0.0_f32);
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::ProgressIndicator);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node.set_busy();
        node
    }));
    let before = create_memo(clone!(phase -> move || {
        ItemSize::Percent(phase.get() * (1.0 - SEGMENT) * 100.0 + 0.001)
    }));
    let after = create_memo(move || {
        ItemSize::Percent((1.0 - phase.get()) * (1.0 - SEGMENT) * 100.0 + 0.001)
    });
    let placed = component_placed();
    let ticking = create_timer(clone!(placed -> move || {
        if !placed.get_untracked() {
            return None;
        }
        set_phase.set((now().saturating_duration_since(started).as_secs_f32() / PERIOD).fract());
        Some(FRAME)
    }));
    create_effect(move || match placed.get() {
        true => ticking.start(Duration::ZERO),
        false => ticking.stop(),
    });
    let width = create_memo(move || Some(width.get()));
    let theme = use_theme();
    view! {
        <Frame width height=HEIGHT color={theme.track.clone()} radius=RADIUS>
            <List direction=Direction::Horizontal spacing=0.0>
                <Spacer @sizing={before} />
                <Frame
                    @sizing=ItemSize::Percent(SEGMENT * 100.0)
                    color={theme.accent.clone()}
                    radius=RADIUS
                />
                <Spacer @sizing={after} />
            </List>
        </Frame>
    }
}
