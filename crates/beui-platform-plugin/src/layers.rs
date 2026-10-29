use beui_core::color::Color32;
use beui_core::context::{Context, EscapedOutput};
use beui_core::escape::{HostTransform, LayerEdge, LayerPlacement, LayerSpace};
use beui_core::geometry::Pos2;
use beui_core::input::Input;
use beui_core::window::WindowId;
use beui_platform_plugin_api::anchored_view;
use block_plugin_api::accessibility::Color;
use block_plugin_api::{AnchoredPlacement, AnchoredView, Scrim, ViewAnchor, ViewEdge, ViewId};

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

pub(crate) fn report(held: &[Shown], owner: ViewId, ratio: f32) -> Vec<AnchoredView> {
    held.iter()
        .map(|shown| {
            let layer = &shown.escaped.layer;
            AnchoredView {
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
