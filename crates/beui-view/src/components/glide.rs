use crate::reactive::{Child, Get, Prop, create_animation, create_effect, with_document};
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Glide(
    #[prop(default = 0)] arm: Prop<u64>,
    #[prop(default = None)] from: Prop<Option<Pos2>>,
    children: Child,
) -> NodeId {
    let glide = with_document(|document| document.create_glide(children));
    let frames = create_animation(
        move |frame| {
            with_document(|document| document.step_glide(glide, frame.delta.as_secs_f32()))
        },
        move || with_document(|document| document.settle_glide(glide)),
    );
    with_document(|document| document.set_glide_on_start(glide, move |()| frames.start()));
    create_effect(move || {
        arm.get();
        with_document(|document| document.arm_glide(glide));
    });
    create_effect(move || {
        if let Some(from) = from.get() {
            with_document(|document| document.glide_from(glide, from));
        }
    });
    glide.id()
}
