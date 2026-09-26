use std::time::{Duration, Instant};

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{
    Direction, Frame, ItemSize, List, Prop, Spacer, clone, component_accessibility, create_effect,
    create_memo, create_signal, create_timer, node_placed,
};
use crate::styled::theme::use_theme;

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
    let started = Instant::now();
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
    let width = create_memo(move || Some(width.get()));
    let theme = use_theme();
    let node = view! {
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
    };
    let placed = node_placed(node);
    let ticking = create_timer(clone!(placed -> move || {
        if !placed.get_untracked() {
            return None;
        }
        set_phase.set((started.elapsed().as_secs_f32() / PERIOD).fract());
        Some(FRAME)
    }));
    create_effect(move || match placed.get() {
        true => ticking.start(Duration::ZERO),
        false => ticking.stop(),
    });
    node
}
