use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::NodeId;

pub(crate) fn measure(doc: &mut Document, painter: &Painter, id: NodeId, available: Vec2) -> Vec2 {
    if !doc.arena.contains(id) {
        return Vec2::ZERO;
    }
    doc.deliver_constraint(id, available);
    if let Some(size) = doc.measured(id, available) {
        return size;
    }
    let element = doc.arena.take(id);
    let size = element.measure(doc, painter, available);
    doc.arena.put_back(id, element);
    doc.remember_measurement(id, available, size);
    size
}

pub(crate) fn layout(
    doc: &mut Document,
    painter: &Painter,
    id: NodeId,
    rect: Rect,
    out: &mut HashMap<NodeId, Rect>,
) {
    if !doc.arena.contains(id) {
        return;
    }
    out.insert(id, rect);
    let watermark = doc.arena.changed_len();
    doc.deliver_placement(id, rect);
    doc.assert_confined(id, watermark, out);
    if !doc.arena.contains(id) {
        return;
    }
    let element = doc.arena.take(id);
    element.layout(doc, painter, rect, out);
    doc.arena.put_back(id, element);
}
