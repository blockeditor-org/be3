use beui_core::color::Color32;
use beui_core::context::{Context, EscapedOutput};
use beui_core::escape::{HostTransform, LayerEdge, LayerPlacement, LayerSpace};
use beui_core::geometry::Pos2;
use beui_core::input::Input;
use beui_core::window::WindowId;
use block_plugin_api::accessibility::Color;
use block_plugin_api::{Layer, LayerId, LayerParent, Scrim};

use crate::surface::Surface;

pub(crate) struct Shown {
    pub(crate) escaped: EscapedOutput,
    pub(crate) surface: Surface,
}

pub(crate) fn set_space(
    context: &Context,
    window: WindowId,
    space: Option<block_plugin_api::LayerSpace>,
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

pub(crate) fn follow(held: &mut Vec<Shown>, escaped: &[EscapedOutput], window: WindowId) -> bool {
    let mut changed = false;
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
            None => {
                changed = true;
                Shown {
                    escaped: escaped.clone(),
                    surface: Surface::default(),
                }
            }
        };
        changed |= escaped.output.changed || shown.escaped.layer != escaped.layer;
        shown.surface.note(&escaped.output);
        shown.escaped = escaped.clone();
        next.push(shown);
    }
    changed |= !held.is_empty();
    *held = next;
    changed
}

pub(crate) fn report(held: &[Shown], ratio: f32) -> Vec<Layer> {
    held.iter()
        .map(|shown| {
            let layer = &shown.escaped.layer;
            let parent = layer.parent.map(LayerId);
            Layer {
                id: LayerId(layer.id),
                parent: parent.map_or(LayerParent::Region, LayerParent::Layer),
                placement: match layer.placement {
                    LayerPlacement::Anchored(rect) => {
                        block_plugin_api::LayerPlacement::Anchored(match parent {
                            Some(_) => rect,
                            None => rect.scaled(ratio.recip()),
                        })
                    }
                    LayerPlacement::Edge(edge) => {
                        block_plugin_api::LayerPlacement::Edge(edge_of(edge))
                    }
                },
                rect: layer.rect,
                surface: 0,
                painted: layer.painted.clone(),
                floating: layer.floating.clone(),
                scrims: layer
                    .scrims
                    .iter()
                    .map(|(rect, color)| scrim(*rect, *color))
                    .collect(),
                dismiss_on_outside_press: layer.dismiss_on_outside_press,
                keyboard: layer.keyboard,
                blocks_owner: layer.blocks_owner,
            }
        })
        .collect()
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

fn edge_of(edge: LayerEdge) -> block_plugin_api::LayerEdge {
    match edge {
        LayerEdge::Top => block_plugin_api::LayerEdge::Top,
        LayerEdge::Bottom => block_plugin_api::LayerEdge::Bottom,
        LayerEdge::Left => block_plugin_api::LayerEdge::Left,
        LayerEdge::Right => block_plugin_api::LayerEdge::Right,
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
