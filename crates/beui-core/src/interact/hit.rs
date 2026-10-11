use std::collections::HashSet;

use crate::base::embed::EmbedNode;
use crate::base::interactive::InteractiveNode;
use crate::base::overlay::OverlayNode;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::Modifiers;
use crate::node::{NodeId, NodeMap, NodeOf, Rects};

use super::TOUCH_REACH;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Hit {
    pub path: Vec<NodeId>,
    pub scrims: Vec<NodeId>,
    pub embed: bool,
    pub shift: Vec2,
}

enum Walk {
    Found(Vec<NodeId>, bool),
    Declined,
    Nothing,
}

enum Layer {
    Modal(NodeOf<OverlayNode>),
    Floating(NodeOf<OverlayNode>),
    Root(NodeId),
}

pub fn interactive(doc: &Document, id: NodeId) -> Option<&InteractiveNode> {
    if !doc.arena.contains(id) {
        return None;
    }
    doc.arena.get(id).as_any().downcast_ref::<InteractiveNode>()
}

pub fn hit(doc: &mut Document, rects: &Rects, root: NodeId, pos: Pos2, touch: bool) -> Hit {
    let mut scrims = Vec::new();
    let mut above: Option<NodeOf<OverlayNode>> = None;
    let found = |hit: Hit, scrims: Vec<NodeId>| Hit { scrims, ..hit };
    for layer in layers(doc, root) {
        match layer {
            Layer::Modal(overlay) if above.is_some_and(|above| !nested(doc, above, overlay)) => {
                break;
            }
            Layer::Modal(overlay) => {
                let content = doc.overlay_content(overlay);
                let inside = content
                    .and_then(|content| rects.visible(&content))
                    .is_some_and(|rect| rect.contains_half_open(pos));
                if let Some(content) = content
                    && let Some(hit) = within(doc, rects, content, pos, touch && inside)
                {
                    return found(hit, scrims);
                }
                if inside {
                    return found(Hit::default(), scrims);
                }
                if !doc.overlay_is_light(overlay) {
                    scrims.push(doc.overlay_scrim(overlay));
                    above = Some(overlay);
                }
            }
            Layer::Floating(_) | Layer::Root(_) if !scrims.is_empty() => {
                let scrim = scrims.pop().expect("a scrim was passed");
                let hit = within(doc, rects, scrim, pos, false).unwrap_or_default();
                return found(hit, scrims);
            }
            Layer::Floating(overlay) => {
                let Some(content) = doc.overlay_content(overlay) else {
                    continue;
                };
                match walk(doc, rects, content, pos) {
                    Walk::Found(path, embed) => {
                        return reach(doc, rects, content, pos, touch, path, embed);
                    }
                    Walk::Declined => continue,
                    Walk::Nothing => {
                        if let Some(hit) = reached(doc, rects, content, pos, touch) {
                            return hit;
                        }
                        if rects
                            .visible(&content)
                            .is_some_and(|rect| rect.contains_half_open(pos))
                        {
                            return Hit::default();
                        }
                    }
                }
            }
            Layer::Root(root) => {
                return within(doc, rects, root, pos, touch).unwrap_or_default();
            }
        }
    }
    match scrims.pop() {
        Some(scrim) => found(within(doc, rects, scrim, pos, false).unwrap_or_default(), scrims),
        None => Hit::default(),
    }
}

fn nested(doc: &Document, inner: NodeOf<OverlayNode>, outer: NodeOf<OverlayNode>) -> bool {
    let mut next = doc.arena.parent(inner.id());
    while let Some(id) = next {
        if id == outer.id() {
            return true;
        }
        next = doc.arena.parent(id);
    }
    false
}

pub fn claimant(
    doc: &mut Document,
    rects: &Rects,
    root: NodeId,
    pos: Pos2,
    held: Modifiers,
) -> Option<NodeId> {
    let mut lines = HashSet::new();
    for claimant in doc.press_claimants.iter().copied() {
        let mut next = Some(claimant);
        while let Some(id) = next {
            if !lines.insert(id) {
                break;
            }
            next = doc.arena.parent(id);
        }
    }
    for layer in layers(doc, root) {
        let (top, light) = match layer {
            Layer::Modal(overlay) => (doc.overlay_content(overlay), doc.overlay_is_light(overlay)),
            Layer::Floating(overlay) => (doc.overlay_content(overlay), true),
            Layer::Root(root) => (Some(root), false),
        };
        let Some(top) = top else {
            continue;
        };
        if let Some(found) = claiming(doc, rects, top, pos, held, &lines) {
            return Some(found);
        }
        let covered = rects
            .visible(&top)
            .is_some_and(|rect| rect.contains_half_open(pos));
        if covered || !light {
            return None;
        }
    }
    None
}

fn claiming(
    doc: &mut Document,
    rects: &Rects,
    id: NodeId,
    pos: Pos2,
    held: Modifiers,
    lines: &HashSet<NodeId>,
) -> Option<NodeId> {
    if !doc.arena.contains(id) || doc.is_culled(id) {
        return None;
    }
    if !lines.contains(&id) && !bounds(doc, rects, id).contains_half_open(pos) {
        return None;
    }
    if let (Some(node), Some(rect)) = (interactive(doc, id), rects.visible(&id))
        && node.claims(pos, rect, held)
    {
        return Some(id);
    }
    doc.arena
        .get(id)
        .pointer_children()
        .into_iter()
        .rev()
        .filter(|child| rects.contains_key(child))
        .find_map(|child| claiming(doc, rects, child, pos, held, lines))
}

pub fn sink(doc: &Document, rects: &Rects, path: &[NodeId], pos: Pos2) -> Option<NodeId> {
    for id in path {
        let Some(node) = interactive(doc, *id) else {
            continue;
        };
        if node.forwards() {
            return Some(*id);
        }
        if rects
            .visible(id)
            .is_some_and(|rect| node.captures(pos, rect))
        {
            return None;
        }
    }
    None
}

fn layers(doc: &Document, root: NodeId) -> Vec<Layer> {
    let mut layers: Vec<Layer> = doc
        .overlays_bottom_up()
        .into_iter()
        .rev()
        .filter_map(|overlay| {
            if doc.overlay_stack.contains(&overlay) {
                Some(Layer::Modal(overlay))
            } else if doc.overlay_is_floating(overlay) {
                Some(Layer::Floating(overlay))
            } else {
                None
            }
        })
        .collect();
    layers.push(Layer::Root(root));
    layers
}

fn within(doc: &mut Document, rects: &Rects, top: NodeId, pos: Pos2, touch: bool) -> Option<Hit> {
    match walk(doc, rects, top, pos) {
        Walk::Found(path, embed) => Some(reach(doc, rects, top, pos, touch, path, embed)),
        Walk::Declined | Walk::Nothing => reached(doc, rects, top, pos, touch),
    }
}

fn reach(
    doc: &mut Document,
    rects: &Rects,
    top: NodeId,
    pos: Pos2,
    touch: bool,
    path: Vec<NodeId>,
    embed: bool,
) -> Hit {
    let direct = path
        .iter()
        .any(|id| interactive(doc, *id).is_some_and(InteractiveNode::claims_touches));
    if touch
        && !direct
        && let Some(hit) = reached(doc, rects, top, pos, touch)
    {
        return hit;
    }
    Hit {
        path,
        scrims: Vec::new(),
        embed,
        shift: Vec2::ZERO,
    }
}

fn reached(doc: &mut Document, rects: &Rects, top: NodeId, pos: Pos2, touch: bool) -> Option<Hit> {
    if !touch {
        return None;
    }
    let mut nearest: Option<(f32, Pos2)> = None;
    nearest_press(doc, rects, top, pos, Rect::EVERYTHING, &mut nearest);
    let (_, at) = nearest?;
    match walk(doc, rects, top, at) {
        Walk::Found(path, embed) => Some(Hit {
            path,
            scrims: Vec::new(),
            embed,
            shift: at - pos,
        }),
        Walk::Declined | Walk::Nothing => None,
    }
}

fn walk(doc: &mut Document, rects: &Rects, id: NodeId, pos: Pos2) -> Walk {
    if !doc.arena.contains(id) || doc.is_culled(id) {
        return Walk::Nothing;
    }
    if !bounds(doc, rects, id).contains_half_open(pos) {
        return Walk::Nothing;
    }
    let mut declined = false;
    for child in doc.arena.get(id).pointer_children().into_iter().rev() {
        if !rects.contains_key(&child) {
            continue;
        }
        match walk(doc, rects, child, pos) {
            Walk::Found(mut path, embed) => {
                if let (Some(node), Some(rect)) = (interactive(doc, id), rects.get(&id))
                    && node.hits(pos, rect)
                {
                    path.push(id);
                }
                return Walk::Found(path, embed);
            }
            Walk::Declined => declined = true,
            Walk::Nothing => {}
        }
    }
    let inside = rects
        .visible(&id)
        .is_some_and(|rect| rect.contains_half_open(pos));
    if inside && let (Some(node), Some(rect)) = (interactive(doc, id), rects.get(&id)) {
        if !node.hits(pos, rect) {
            return Walk::Declined;
        }
        if declined && !node.forwards() && !node.captures(pos, rect) {
            return Walk::Declined;
        }
        return Walk::Found(vec![id], false);
    }
    if inside && doc.arena.get(id).as_any().is::<EmbedNode>() {
        return Walk::Found(Vec::new(), true);
    }
    match declined {
        true => Walk::Declined,
        false => Walk::Nothing,
    }
}

fn bounds(doc: &mut Document, rects: &Rects, id: NodeId) -> Rect {
    if doc.interact_bounds_version != Some(rects.version()) {
        doc.interact_bounds = NodeMap::default();
        doc.interact_bounds_version = Some(rects.version());
    }
    if let Some(bounds) = doc.interact_bounds.get(&id) {
        return *bounds;
    }
    let Some(rect) = rects.get(&id) else {
        return Rect::NOTHING;
    };
    doc.interact_bounds.insert(id, rect);
    let mut bounds = rect;
    for child in doc.arena.get(id).pointer_children() {
        if rects.contains_key(&child) && doc.arena.contains(child) {
            bounds = bounds.union(self::bounds(doc, rects, child));
        }
    }
    doc.interact_bounds.insert(id, bounds);
    bounds
}

fn nearest_press(
    doc: &Document,
    rects: &Rects,
    id: NodeId,
    pos: Pos2,
    clip: Rect,
    nearest: &mut Option<(f32, Pos2)>,
) {
    if !doc.arena.contains(id) || doc.is_culled(id) {
        return;
    }
    let Some(rect) = rects.visible(&id).map(|rect| rect.intersect(clip)) else {
        return;
    };
    if !rect.is_positive() || !rect.expand(TOUCH_REACH).contains(pos) {
        return;
    }
    if interactive(doc, id).is_some_and(InteractiveNode::claims_touches) {
        let at = Pos2::new(
            pos.x.clamp(rect.left(), rect.right().next_down()),
            pos.y.clamp(rect.top(), rect.bottom().next_down()),
        );
        let away = at.distance(pos);
        if away <= TOUCH_REACH && nearest.is_none_or(|(held, _)| away < held) {
            *nearest = Some((away, at));
        }
    }
    for child in doc.arena.get(id).pointer_children() {
        nearest_press(doc, rects, child, pos, rect, nearest);
    }
}
