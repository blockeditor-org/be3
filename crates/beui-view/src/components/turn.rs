use crate::reactive::{Child, Get, Prop, create_effect, with_document};
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Turn(
    #[prop(default = 0)] quarter_turns: Prop<u8>,
    #[prop(default = 0.0)] angle: Prop<f32>,
    children: Child,
) -> NodeId {
    let turn = with_document(|document| document.create_turn(children));
    create_effect(move || {
        let quarters = quarter_turns.get();
        with_document(|document| document.set_turn(turn, quarters));
    });
    create_effect(move || {
        let angle = angle.get();
        with_document(|document| document.set_turn_angle(turn, angle));
    });
    turn.id()
}
