use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::theme::use_theme;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const HEIGHT: f32 = 6.0;
const RADIUS: f32 = 3.0;

#[component]
pub fn Progress(value: Prop<f32>, #[prop(default = String::new())] label: Prop<String>) -> NodeId {
    let value = value.map(|value| value.clamp(0.0, 1.0));
    let value_read = create_memo(move || value.get());

    let filled = create_memo(move || Extent::fraction(value_read.get()));
    component_accessibility(create_memo(move || {
        let value = value_read.get();
        let mut node = Node::new(Role::ProgressIndicator);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node.set_numeric_value(value.into());
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node
    }));
    let theme = use_theme();

    view! {
        <Frame
            height=HEIGHT
            width={Extent::fraction(1.0)}
            fill={theme.track}
            radius=RADIUS
            align={Align2::horizontal(Align::Start)}
        >
            <Frame width=filled fill={theme.accent} radius=RADIUS />
        </Frame>
    }
}
