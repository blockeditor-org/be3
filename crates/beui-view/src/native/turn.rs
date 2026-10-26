use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::node::NodeId;

pub fn turn(
    quarter_turns: Prop<u8>,
    angle: Prop<f32>,
    children: Child
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
