use beui_core::color::Color32;
use beui_core::context::{Context, EscapedOutput};
use beui_core::escape::{HostTransform, LayerEdge, LayerPlacement, LayerSpace};
use beui_core::geometry::Pos2;
use beui_core::input::Input;
use beui_core::window::WindowId;
use beui_platform_plugin_api::anchored_view;
use block_editor_plugin::{Anchored, Frame};
use block_plugin_api::accessibility::Color;
use block_plugin_api::{AnchoredPlacement, Scrim, ViewAnchor, ViewEdge, ViewId};

use block_editor_plugin::driver::Surface;

use crate::surface::{Painter, note};

pub(crate) struct Shown {
    pub(crate) escaped: EscapedOutput,
    pub(crate) surface: Surface<Painter>,
    pub(crate) changed: bool,
}

pub(crate) fn set_space(
    context: &Context,
    window: WindowId,
    space: Option<block_plugin_api::AnchorSpace>,
    ratio: f32,
) {
    let space = space.map(|space| LayerSpace {
        to_host: HostTransform {
            origin: space.to_host.origin,
            scale: space.to_host.scale / ratio,
        },
        bounds: space.bounds,
    });
    if context.layer_space(window) != space {
        context.set_layer_space(window, space);
    }
}

pub(crate) fn follow(held: &mut Vec<Shown>, escaped: &[EscapedOutput], window: WindowId) {
    let mut next = Vec::new();
    for escaped in escaped
        .iter()
        .filter(|escaped| escaped.layer.window == window)
    {
        let at = held
            .iter()
            .position(|shown| shown.escaped.layer.id == escaped.layer.id);
        let mut shown = match at {
            Some(at) => held.remove(at),
            None => Shown {
                escaped: escaped.clone(),
                surface: Surface::default(),
                changed: true,
            },
        };
        shown.changed |= escaped.output.changed || shown.escaped.layer != escaped.layer;
        note(
            &mut shown.surface,
            &escaped.output,
            beui_core::geometry::Vec2::ZERO,
        );
        shown.escaped = escaped.clone();
        next.push(shown);
    }
    *held = next;
}

pub(crate) fn report(held: &[Shown], owner: ViewId, ratio: f32) -> Vec<Anchored> {
    held.iter()
        .map(|shown| {
            let layer = &shown.escaped.layer;
            Anchored {
                view: anchored_view(layer.id),
                placement: AnchoredPlacement {
                    parent: layer.parent.map_or(owner, anchored_view),
                    anchor: match layer.placement {
                        LayerPlacement::Anchored(rect) => ViewAnchor::Rect(match layer.parent {
                            Some(_) => rect,
                            None => rect.scaled(ratio.recip()),
                        }),
                        LayerPlacement::Edge(edge) => ViewAnchor::Edge(edge_of(edge)),
                    },
                    rect: layer.rect,
                    dismiss_on_outside_press: layer.dismiss_on_outside_press,
                    keyboard: layer.keyboard,
                    blocks_owner: layer.blocks_owner,
                },
            }
        })
        .collect()
}

pub(crate) fn framed(
    shown: &mut Shown,
    description: Option<block_editor_plugin::Description>,
) -> Frame {
    let layer = &shown.escaped.layer;
    let from = layer.rect.min.to_vec2();
    let own = |rects: &[beui_core::geometry::Rect]| -> Vec<beui_core::geometry::Rect> {
        rects.iter().map(|rect| rect.translate(-from)).collect()
    };
    Frame {
        changed: std::mem::take(&mut shown.changed),
        painted: own(&layer.painted),
        floating: own(&layer.floating),
        scrims: layer
            .scrims
            .iter()
            .map(|(rect, color)| scrim(rect.translate(-from), *color))
            .collect(),
        description,
        ..Frame::default()
    }
}

fn scrim(rect: beui_core::geometry::Rect, color: Color32) -> Scrim {
    let [red, green, blue, alpha] = color.to_array();
    Scrim {
        rect,
        color: Color {
            red,
            green,
            blue,
            alpha,
        },
    }
}

fn edge_of(edge: LayerEdge) -> ViewEdge {
    match edge {
        LayerEdge::Top => ViewEdge::Top,
        LayerEdge::Bottom => ViewEdge::Bottom,
        LayerEdge::Left => ViewEdge::Left,
        LayerEdge::Right => ViewEdge::Right,
    }
}

pub(crate) fn to_host(input: Input, to_host: block_plugin_api::HostTransform) -> Input {
    let at = |pos: Pos2| to_host.point(pos);
    match input {
        Input::PointerMoved(pos) => Input::PointerMoved(at(pos)),
        Input::PointerButton {
            pos,
            button,
            pressed,
        } => Input::PointerButton {
            pos: at(pos),
            button,
            pressed,
        },
        Input::PointerMotion(delta) => Input::PointerMotion(beui_core::geometry::vec2(
            delta.x * to_host.scale.x,
            delta.y * to_host.scale.y,
        )),
        Input::Touch {
            id,
            phase,
            pos,
            force,
        } => Input::Touch {
            id,
            phase,
            pos: at(pos),
            force,
        },
        Input::Pen { phase, pos, pen } => Input::Pen {
            phase,
            pos: at(pos),
            pen,
        },
        Input::Predicted(pos) => Input::Predicted(at(pos)),
        other => other,
    }
}
