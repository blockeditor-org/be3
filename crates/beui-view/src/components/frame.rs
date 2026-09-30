use beui_macros::component;

use crate::reactive::{Child, Prop, create_effect, with_document};
use beui_core::base::frame::FrameStyle;
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::node::NodeId;

fn bind_measurement(
    frame: NodeId,
    measurement: Prop<Option<f32>>,
    set: fn(&mut Document, NodeId, Option<f32>),
) {
    match measurement {
        Prop::Static(None) => {}
        Prop::Static(value) => with_document(|document| set(document, frame, value)),
        Prop::Dynamic(read) => {
            create_effect(move || {
                let value = read();
                with_document(|document| set(document, frame, value));
            });
        }
    }
}

#[component]
pub fn Frame(
    #[prop(default = None)] width: Prop<Option<f32>>,
    #[prop(default = None)] max_width: Prop<Option<f32>>,
    #[prop(default = None)] height: Prop<Option<f32>>,
    #[prop(default = None)] min_height: Prop<Option<f32>>,
    #[prop(default = None)] aspect_ratio: Prop<Option<f32>>,
    #[prop(default = 0.0)] padding_horizontal: Prop<f32>,
    #[prop(default = 0.0)] padding_vertical: Prop<f32>,
    #[prop(default = Color32::TRANSPARENT)] color: Prop<Color32>,
    #[prop(default = Color32::TRANSPARENT)] outline: Prop<Color32>,
    #[prop(default = 0.0)] outline_width: Prop<f32>,
    #[prop(default = 0)] radius: Prop<u8>,
    #[prop(default = 0.0)] outline_offset: Prop<f32>,
    #[prop(default = false)] outline_visible: Prop<bool>,
    #[prop(default = true)] visible: Prop<bool>,
    children: Option<Child>,
) -> NodeId {
    let frame = with_document(|document| {
        let frame = document.create_frame();
        if let Some(child) = children {
            document.set_frame_child(frame, child);
        }
        frame
    });
    bind_measurement(frame, width, Document::set_frame_width);
    bind_measurement(frame, max_width, Document::set_frame_max_width);
    bind_measurement(frame, height, Document::set_frame_height);
    bind_measurement(frame, min_height, Document::set_frame_min_height);
    bind_measurement(frame, aspect_ratio, Document::set_frame_aspect_ratio);
    create_effect(move || {
        with_document(|document| {
            document.set_frame_padding(frame, padding_horizontal.get(), padding_vertical.get());
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_frame_style(
                frame,
                FrameStyle {
                    fill: color.get(),
                    outline: outline.get(),
                    outline_width: outline_width.get(),
                    radius: radius.get(),
                    outline_offset: outline_offset.get(),
                    outline_visible: outline_visible.get(),
                },
            );
        })
    });
    create_effect(move || with_document(|document| document.set_visible(frame, visible.get())));
    frame
}
