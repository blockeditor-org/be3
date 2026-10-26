use beui_macros::component;

use crate::reactive::{Child, Get, Prop, create_binding, with_document};
use beui_core::base::frame::{Align2, Extent, FrameNode, FrameStyle, Outline, Sides};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::node::{NodeId, NodeOf};
use beui_core::painter::Corners;

fn bind<T: Clone + 'static>(
    frame: NodeOf<FrameNode>,
    prop: Prop<T>,
    set: fn(&mut Document, NodeOf<FrameNode>, T),
) {
    match prop {
        Prop::Static(value) => with_document(|document| set(document, frame, value)),
        Prop::Dynamic(read) => create_binding(move || {
            let value = read.get();
            with_document(|document| set(document, frame, value));
        }),
    }
}

#[component]
pub fn Frame(
    #[prop(default = Extent::INTRINSIC)] width: Prop<Extent>,
    #[prop(default = Extent::INTRINSIC)] height: Prop<Extent>,
    #[prop(default = None)] aspect_ratio: Prop<Option<f32>>,
    #[prop(default = Sides::ZERO)] padding: Prop<Sides>,
    #[prop(default = Align2::STRETCH)] align: Prop<Align2>,
    #[prop(default = Color32::TRANSPARENT)] fill: Prop<Color32>,
    #[prop(default = Corners::ZERO)] radius: Prop<Corners>,
    #[prop(default = None)] outline: Prop<Option<Outline>>,
    #[prop(default = 0.0)] pointer: Prop<f32>,
    children: Option<Child>,
) -> NodeId {
    let frame = with_document(|document| {
        let frame = document.create_frame();
        if let Some(child) = children {
            document.set_frame_child(frame, child);
        }
        frame
    });
    bind(frame, width, Document::set_frame_width);
    bind(frame, height, Document::set_frame_height);
    bind(frame, aspect_ratio, Document::set_frame_aspect_ratio);
    bind(frame, padding, Document::set_frame_padding);
    bind(frame, align, Document::set_frame_align);
    create_binding(move || {
        let style = FrameStyle {
            fill: fill.get(),
            radius: radius.get(),
            outline: outline.get(),
            pointer: pointer.get(),
        };
        with_document(|document| document.set_frame_style(frame, style));
    });
    frame.id()
}
