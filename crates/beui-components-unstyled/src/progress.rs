use accesskit::{Node, Role};
use beui_macros::component;

use beui_view::reactive::NodeId;
use beui_view::reactive::*;

const PERIOD: f32 = 1.2;
const RESTING_PHASE: f32 = 0.5;

pub struct ProgressHandle {
    pub value: Memo<Option<f32>>,
    pub phase: ReadSignal<f32>,
}

#[component]
pub fn Progress(
    value: Prop<Option<f32>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(children)] content: Render<ProgressHandle>,
) -> NodeId {
    let value = create_memo(move || value.get().map(|value| value.clamp(0.0, 1.0)));
    let indeterminate = create_memo(move || value.get().is_none());
    let phase = create_signal(RESTING_PHASE);
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::ProgressIndicator);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        match value.get() {
            Some(value) => {
                node.set_numeric_value(value.into());
                node.set_min_numeric_value(0.0);
                node.set_max_numeric_value(1.0);
            }
            None => node.set_busy(),
        }
        node
    }));
    let placed = component_placed();
    let ticking = create_animation(
        move |frame| {
            if !placed.get_untracked() {
                return false;
            }
            phase.set((frame.elapsed.as_secs_f32() / PERIOD).fract());
            true
        },
        move || phase.set(RESTING_PHASE),
    );
    let preferences = use_preferences();
    let animates = create_memo(move || preferences.get().motion.animates());
    create_effect(move || {
        if indeterminate.get() && placed.get() && animates.get() {
            ticking.start();
        } else {
            ticking.stop();
            phase.set(RESTING_PHASE);
        }
    });
    content.call(ProgressHandle {
        value,
        phase: phase.read_only(),
    })
}
