use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::reactive::with_document;
use beui_core::base::frame::Align2;
use beui_core::base::frame::Extent;
use beui_core::base::frame::FrameNode;
use beui_core::base::frame::FrameStyle;
use beui_core::base::frame::Outline;
use beui_core::base::frame::Sides;
use beui_core::color::Color;
use beui_core::document::Document;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;
use beui_core::node::NodeOf;
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

pub fn frame(
    width: Prop<Extent>,
    height: Prop<Extent>,
    aspect_ratio: Prop<Option<f32>>,
    padding: Prop<Sides>,
    align: Prop<Align2>,
    fill: Prop<Color>,
    radius: Prop<Corners>,
    outline: Prop<Option<Outline>>,
    translate: Prop<Vec2>,
    rotate: Prop<f32>,
    opacity: Prop<f32>,
    fade_edges: Prop<Sides>,
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
    bind(frame, translate, Document::set_frame_translate);
    bind(frame, rotate, Document::set_frame_rotate);
    bind(frame, opacity, Document::set_frame_opacity);
    bind(frame, fade_edges, Document::set_frame_fade_edges);
    create_binding(move || {
        let style = FrameStyle {
            fill: fill.get(),
            radius: radius.get(),
            outline: outline.get(),
        };
        with_document(|document| document.set_frame_style(frame, style));
    });
    frame.id()
}
