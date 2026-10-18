use crate::frame::{ColorRole, Frame, Radius};

use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const HEIGHT: f32 = 4.0;
const RADIUS: f32 = 2.0;
const SEGMENT: f32 = 0.35;
const PERIOD: f32 = 1.2;

#[component]
pub fn Spinner(
    #[prop(default = 40.0)] width: Prop<f32>,
    #[prop(default = String::new())] label: Prop<String>,
) -> NodeId {
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
    let before =
        create_memo(move || ItemSize::Percent(phase.get() * (1.0 - SEGMENT) * 100.0 + 0.001));
    let after = create_memo(move || {
        ItemSize::Percent((1.0 - phase.get()) * (1.0 - SEGMENT) * 100.0 + 0.001)
    });
    let placed = component_placed();
    let ticking = create_animation(
        move |frame| {
            if !placed.get_untracked() {
                return false;
            }
            set_phase.set((frame.elapsed.as_secs_f32() / PERIOD).fract());
            true
        },
        move || set_phase.set(0.5),
    );
    let preference = with_document(|document| document.watch_motion());
    create_effect(move || {
        let animate = preference.get().animates();
        if placed.get() && animate {
            ticking.start();
        } else {
            ticking.stop();
            set_phase.set(0.5);
        }
    });
    let width = create_memo(move || Some(width.get()));
    view! {
        <Frame width=width height=HEIGHT fill=ColorRole::Track radius={Radius::custom(RADIUS)}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Spacer @sizing={before} />
                <Frame
                    @sizing=ItemSize::Percent(SEGMENT * 100.0)
                    fill=ColorRole::Accent
                    radius={Radius::custom(RADIUS)}
                />
                <Spacer @sizing={after} />
            </List>
        </Frame>
    }
}
