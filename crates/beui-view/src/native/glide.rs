use crate::reactive::{Child, Get, NodeId, Prop, create_animation, create_effect};
use crate::types::{GlideHold, Pos2, Vec2};
use beui_core::base::glide::GlideNode;
use beui_core::current::with_document;

pub(crate) fn glide(arm: Prop<u64>, from: Prop<Option<Pos2>>, children: Child) -> NodeId {
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

pub(crate) fn hold_glide(glide: NodeId, hold: Option<GlideHold>) {
    with_document(|document| {
        if let Some(glide) = document.arena.kind_of::<GlideNode>(glide) {
            document.hold_glide(glide, hold);
        }
    });
}

pub(crate) fn glide_displacement(glide: NodeId) -> Option<Vec2> {
    with_document(|document| {
        document
            .arena
            .kind_of::<GlideNode>(glide)
            .map(|glide| document.glide_displacement(glide))
    })
}

pub(crate) fn set_raised(node: NodeId, raised: bool) {
    with_document(|document| document.set_raised(node, raised));
}
