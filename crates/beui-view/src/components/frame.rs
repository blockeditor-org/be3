use beui_macros::component;

use crate::reactive::{Child, Prop, create_effect, with_document};
use beui_core::base::Align;
use beui_core::base::frame::FrameNode;
use beui_core::base::frame::{FrameStyle, Sides};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::node::{NodeId, NodeOf};
use beui_core::painter::Corners;

fn bind_measurement(
    frame: NodeOf<FrameNode>,
    measurement: Prop<Option<f32>>,
    set: fn(&mut Document, NodeOf<FrameNode>, Option<f32>),
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
    #[prop(default = None)] min_width: Prop<Option<f32>>,
    #[prop(default = None)] max_width: Prop<Option<f32>>,
    #[prop(default = None)] width_fraction: Prop<Option<f32>>,
    #[prop(default = None)] height: Prop<Option<f32>>,
    #[prop(default = None)] min_height: Prop<Option<f32>>,
    #[prop(default = None)] max_height: Prop<Option<f32>>,
    #[prop(default = None)] height_fraction: Prop<Option<f32>>,
    #[prop(default = None)] aspect_ratio: Prop<Option<f32>>,
    #[prop(default = 0.0)] padding_horizontal: Prop<f32>,
    #[prop(default = 0.0)] padding_vertical: Prop<f32>,
    #[prop(default = None)] padding_left: Prop<Option<f32>>,
    #[prop(default = None)] padding_top: Prop<Option<f32>>,
    #[prop(default = None)] padding_right: Prop<Option<f32>>,
    #[prop(default = None)] padding_bottom: Prop<Option<f32>>,
    #[prop(default = Align::Stretch)] align_horizontal: Prop<Align>,
    #[prop(default = Align::Stretch)] align_vertical: Prop<Align>,
    #[prop(default = Color32::TRANSPARENT)] color: Prop<Color32>,
    #[prop(default = Color32::TRANSPARENT)] outline: Prop<Color32>,
    #[prop(default = 0.0)] outline_width: Prop<f32>,
    #[prop(default = 0)] radius: Prop<u8>,
    #[prop(default = None)] radius_top_left: Prop<Option<u8>>,
    #[prop(default = None)] radius_top_right: Prop<Option<u8>>,
    #[prop(default = None)] radius_bottom_right: Prop<Option<u8>>,
    #[prop(default = None)] radius_bottom_left: Prop<Option<u8>>,
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
    bind_measurement(frame, min_width, Document::set_frame_min_width);
    bind_measurement(frame, max_width, Document::set_frame_max_width);
    bind_measurement(frame, width_fraction, Document::set_frame_width_fraction);
    bind_measurement(frame, height, Document::set_frame_height);
    bind_measurement(frame, min_height, Document::set_frame_min_height);
    bind_measurement(frame, max_height, Document::set_frame_max_height);
    bind_measurement(frame, height_fraction, Document::set_frame_height_fraction);
    bind_measurement(frame, aspect_ratio, Document::set_frame_aspect_ratio);
    create_effect(move || {
        let horizontal = padding_horizontal.get();
        let vertical = padding_vertical.get();
        let padding = Sides {
            left: padding_left.get().unwrap_or(horizontal),
            top: padding_top.get().unwrap_or(vertical),
            right: padding_right.get().unwrap_or(horizontal),
            bottom: padding_bottom.get().unwrap_or(vertical),
        };
        with_document(|document| document.set_frame_padding(frame, padding));
    });
    create_effect(move || {
        let horizontal = align_horizontal.get();
        let vertical = align_vertical.get();
        with_document(|document| document.set_frame_content_align(frame, horizontal, vertical));
    });
    create_effect(move || {
        let all = radius.get();
        let corner = |radius: Option<u8>| f32::from(radius.unwrap_or(all));
        let radius = Corners {
            top_left: corner(radius_top_left.get()),
            top_right: corner(radius_top_right.get()),
            bottom_right: corner(radius_bottom_right.get()),
            bottom_left: corner(radius_bottom_left.get()),
        };
        with_document(|document| {
            document.set_frame_style(
                frame,
                FrameStyle {
                    fill: color.get(),
                    outline: outline.get(),
                    outline_width: outline_width.get(),
                    radius,
                    outline_offset: outline_offset.get(),
                    outline_visible: outline_visible.get(),
                },
            );
        })
    });
    create_effect(move || with_document(|document| document.set_visible(frame, visible.get())));
    frame.id()
}
