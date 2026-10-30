use stable_hash::HashSet;

use crate::base::embed::EmbedNode;
use crate::base::interactive::{InteractiveNode, local_in};
use crate::base::overlay::OverlayNode;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::Modifiers;
use crate::layer::LayerPolicy;
use crate::node::{NodeId, NodeOf, Rects};
use crate::window::WindowId;
use beui_types::region::Area;

use super::TOUCH_REACH;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Hit {
    pub path: Vec<NodeId>,
    pub embed: bool,
    pub shift: Vec2,
    pub blocked: bool,
    pub outside: Vec<NodeOf<OverlayNode>>,
}

enum Walk {
    Found(Vec<NodeId>, bool),
    Declined,
    Nothing,
}

pub(crate) fn covers(doc: &Document, rects: &Rects, id: NodeId, area: &Area, pos: Pos2) -> bool {
    match area {
        Area::Nowhere => false,
        Area::Whole => rects
            .visible(&id)
            .is_some_and(|rect| rect.contains_half_open(pos)),
        Area::Within(region) => rects
            .scaled(&id)
            .is_some_and(|(rect, scale)| region.contains(local_in(rect, scale, pos))),
        Area::CaretHandles => crate::base::text::handle_under(doc, rects, id, pos),
    }
}

fn hits(doc: &Document, rects: &Rects, id: NodeId, pos: Pos2) -> bool {
    interactive(doc, id).is_none_or(|node| match &node.hit_area {
        Area::Whole => true,
        area => covers(doc, rects, id, area, pos),
    })
}

pub(crate) fn captures(doc: &Document, rects: &Rects, id: NodeId, pos: Pos2) -> bool {
    rects.visible(&id).is_some()
        && interactive(doc, id).is_some_and(|node| covers(doc, rects, id, &node.captures, pos))
}

pub(crate) fn draws_at(doc: &Document, rects: &Rects, id: NodeId, pos: Pos2) -> bool {
    rects.visible(&id).is_some()
        && interactive(doc, id).is_some_and(|node| covers(doc, rects, id, &node.drawing, pos))
}

pub(crate) fn takes_gestures_at(doc: &Document, rects: &Rects, id: NodeId, pos: Pos2) -> bool {
    interactive(doc, id).is_some_and(|node| covers(doc, rects, id, &node.gesture_area, pos))
}

fn claims(doc: &Document, rects: &Rects, id: NodeId, pos: Pos2, held: Modifiers) -> bool {
    rects.visible(&id).is_some()
        && interactive(doc, id).is_some_and(|node| {
            node.claims.iter().any(|claim| {
                held.holds(claim.modifiers) && covers(doc, rects, id, &claim.area, pos)
            })
        })
}

pub fn interactive(doc: &Document, id: NodeId) -> Option<&InteractiveNode> {
    if !doc.arena.contains(id) {
        return None;
    }
    doc.arena.get(id).as_any().downcast_ref::<InteractiveNode>()
}

pub fn hit(doc: &mut Document, rects: &Rects, window: WindowId, pos: Pos2, touch: bool) -> Hit {
    let Some((layers, root)) = window_layers(doc, window) else {
        return Hit {
            blocked: true,
            ..Hit::default()
        };
    };
    let mut outside = Vec::new();
    let mut blocking = false;
    let settle =
        |doc: &Document, at: usize, mut hit: Hit, mut outside: Vec<NodeOf<OverlayNode>>| {
            if let Some(blocker) = doc.in_blocked(&layers, at, hit.path.first().copied(), pos) {
                outside.push(blocker);
                return Hit {
                    blocked: true,
                    outside,
                    ..Hit::default()
                };
            }
            hit.outside = outside;
            hit
        };
    for index in (0..layers.len()).rev() {
        let layer = &layers[index];
        if layer.policy == LayerPolicy::Inert {
            continue;
        }
        if blocking {
            if index == 0 {
                break;
            }
            if !layers.reachable(index) {
                continue;
            }
            let inside = layer
                .content
                .and_then(|content| rects.visible(&content))
                .is_some_and(|rect| rect.contains_half_open(pos));
            if inside && let Some(content) = layer.content {
                let hit = within(doc, rects, content, pos, touch).unwrap_or_default();
                return settle(doc, index, hit, outside);
            }
            if !layer.blocks_all() {
                break;
            }
            outside.extend(layer.overlay.filter(|_| layer.dismissable));
            continue;
        }
        let Some(content) = (match index {
            0 => Some(root),
            _ => layer.content,
        }) else {
            continue;
        };
        let inside = rects
            .visible(&content)
            .is_some_and(|rect| rect.contains_half_open(pos));
        if index == 0 {
            let hit = within(doc, rects, content, pos, touch).unwrap_or_default();
            return settle(doc, index, hit, outside);
        }
        if layer.blocks_all() {
            if let Some(hit) = within(doc, rects, content, pos, touch && inside) {
                return settle(doc, index, hit, outside);
            }
            if inside {
                return settle(doc, index, Hit::default(), outside);
            }
            outside.extend(layer.overlay.filter(|_| layer.dismissable));
            blocking = true;
            continue;
        }
        let walked = walk(doc, rects, content, pos);
        if matches!(walked, Walk::Declined) {
            continue;
        }
        if let Some(hit) = reach(doc, rects, content, pos, touch, walked) {
            return settle(doc, index, hit, outside);
        }
        if inside {
            return settle(doc, index, Hit::default(), outside);
        }
        if layer.policy == LayerPolicy::Pass
            && layer.dismissable
            && let Some(overlay) = layer.overlay
            && !doc.on_overlay_trigger(overlay, pos)
        {
            outside.push(overlay);
        }
    }
    Hit {
        blocked: true,
        outside,
        ..Hit::default()
    }
}

fn window_layers(doc: &Document, window: WindowId) -> Option<(crate::layer::Layers, NodeId)> {
    let layers = doc.window_layers(window)?;
    let root = layers[0].content?;
    Some((layers, root))
}

pub fn claimant(
    doc: &mut Document,
    rects: &Rects,
    window: WindowId,
    pos: Pos2,
    held: Modifiers,
) -> Option<NodeId> {
    let (layers, root) = window_layers(doc, window)?;
    let mut lines = HashSet::default();
    for claimant in doc.press_claimants.iter().copied() {
        let mut next = Some(claimant);
        while let Some(id) = next {
            if !lines.insert(id) {
                break;
            }
            next = doc.arena.parent(id);
        }
    }
    for index in (0..layers.len()).rev() {
        let layer = &layers[index];
        if layer.policy == LayerPolicy::Inert {
            continue;
        }
        let top = match index {
            0 => Some(root),
            _ => layer.content,
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
        if covered || layer.blocks_all() {
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
    if claims(doc, rects, id, pos, held) {
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
        if captures(doc, rects, *id, pos) {
            return None;
        }
    }
    None
}

fn within(doc: &mut Document, rects: &Rects, top: NodeId, pos: Pos2, touch: bool) -> Option<Hit> {
    let walked = walk(doc, rects, top, pos);
    reach(doc, rects, top, pos, touch, walked)
}

fn reach(
    doc: &mut Document,
    rects: &Rects,
    top: NodeId,
    pos: Pos2,
    touch: bool,
    walked: Walk,
) -> Option<Hit> {
    let direct = match &walked {
        Walk::Found(path, _) => path
            .iter()
            .any(|id| interactive(doc, *id).is_some_and(InteractiveNode::claims_touches)),
        Walk::Declined | Walk::Nothing => false,
    };
    if touch
        && !direct
        && let Some(hit) = nearest(doc, rects, top, pos)
    {
        return Some(hit);
    }
    match walked {
        Walk::Found(path, embed) => Some(Hit {
            path,
            embed,
            ..Hit::default()
        }),
        Walk::Declined | Walk::Nothing => None,
    }
}

fn nearest(doc: &mut Document, rects: &Rects, top: NodeId, pos: Pos2) -> Option<Hit> {
    let mut nearest: Option<(f32, Pos2)> = None;
    nearest_press(doc, rects, top, pos, Rect::EVERYTHING, &mut nearest);
    let (_, at) = nearest?;
    match walk(doc, rects, top, at) {
        Walk::Found(path, embed) => Some(Hit {
            path,
            embed,
            shift: at - pos,
            ..Hit::default()
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
                if interactive(doc, id).is_some() && rects.scaled(&id).is_some() {
                    match hits(doc, rects, id, pos) {
                        true => path.push(id),
                        false if path.is_empty() => return Walk::Declined,
                        false => {}
                    }
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
    if inside
        && let Some(node) = interactive(doc, id)
        && rects.scaled(&id).is_some()
    {
        if !hits(doc, rects, id, pos) {
            return Walk::Declined;
        }
        if declined && !node.forwards() && !captures(doc, rects, id, pos) {
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
        doc.interact_bounds.clear();
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

pub fn describe(doc: &mut Document, rects: &Rects, pos: Pos2, scale: f32) -> String {
    let found = hit(doc, rects, WindowId::MAIN, pos, false);
    let mut text = String::new();
    if found.blocked {
        text.push_str("blocked by a layer that takes every press\n");
    }
    match (found.path.is_empty(), found.embed) {
        (true, true) => text.push_str("an embed, with no Interactive over it\n"),
        (true, false) => text.push_str("no Interactive\n"),
        (false, true) => {
            text.push_str("over an embed, along these Interactives, innermost first:\n")
        }
        (false, false) => text.push_str("along these Interactives, innermost first:\n"),
    }
    for id in &found.path {
        text.push_str(&format!("  {}\n", described(doc, rects, *id, scale)));
    }
    if let Some(sink) = sink(doc, rects, &found.path, pos) {
        text.push_str(&format!(
            "forwards to {}\n",
            described(doc, rects, sink, scale)
        ));
    }
    for overlay in &found.outside {
        let id = NodeId::from(*overlay);
        text.push_str(&format!("dismisses {}\n", described(doc, rects, id, scale)));
    }
    text
}

fn described(doc: &Document, rects: &Rects, id: NodeId, scale: f32) -> String {
    let mut text = doc.node_kind(id).to_owned();
    if let Some(rect) = rects.get(&id) {
        text.push_str(&format!(
            " at {},{} size {}x{}",
            (rect.min.x * scale).round(),
            (rect.min.y * scale).round(),
            (rect.width() * scale).round(),
            (rect.height() * scale).round(),
        ));
    }
    for test_id in doc.node_test_ids(id) {
        text.push_str(&format!(" #{test_id}"));
    }
    let made: Vec<&str> = std::iter::successors(Some(id), |id| doc.node_parent(*id))
        .flat_map(|id| doc.component_names(id).into_iter())
        .take(8)
        .collect();
    if !made.is_empty() {
        text.push_str(&format!(" in {}", made.join(" in ")));
    }
    if let Some(node) = interactive(doc, id) {
        let handles = handlers(node);
        text.push_str(&match handles.is_empty() {
            true => ", handling nothing".to_owned(),
            false => format!(", handling {}", handles.join(", ")),
        });
    }
    text
}

fn handlers(node: &InteractiveNode) -> Vec<&'static str> {
    [
        (
            "click",
            !node.on_click.is_empty() || !node.on_click_at.is_empty(),
        ),
        ("press", !node.on_press.is_empty()),
        ("secondary press", !node.on_secondary_press.is_empty()),
        ("middle click", !node.on_middle_click.is_empty()),
        ("drag", !node.on_drag.is_empty()),
        ("pointers", !node.on_pointer.is_empty()),
        ("wheel", !node.on_wheel.is_empty()),
        (
            "hover",
            !node.on_hover_change.is_empty() || !node.on_hover_move.is_empty(),
        ),
        ("forwarded input", node.forwards()),
        ("focus", node.focus.is_some()),
        ("capturing presses", node.captures != Area::Nowhere),
        ("its own hit test", node.hit_area != Area::Whole),
    ]
    .into_iter()
    .filter_map(|(name, handles)| handles.then_some(name))
    .collect()
}
