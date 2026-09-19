use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{NodeId, NodeMap};

pub(crate) fn measure(doc: &mut Document, painter: &Painter, id: NodeId, available: Vec2) -> Vec2 {
    if !doc.arena.contains(id) {
        return Vec2::ZERO;
    }
    doc.note_parent(id);
    doc.deliver_constraint(id, available);
    if !doc.arena.contains(id) {
        return Vec2::ZERO;
    }
    if let Some(size) = doc.measured(id, available) {
        return size;
    }
    let watermark = doc.arena.revision;
    let element = doc.arena.take(id);
    let outer = doc.enter_layout(id);
    let measured = element.measure(doc, painter, available);
    doc.leave_layout(outer);
    doc.arena.put_back(id, element);
    let size = doc.pixel_grid().snap_size(measured);
    doc.remember_measurement(id, available, size, watermark);
    size
}

pub(crate) fn layout(
    doc: &mut Document,
    painter: &Painter,
    id: NodeId,
    rect: Rect,
    out: &mut NodeMap<Rect>,
) {
    if !doc.arena.contains(id) {
        return;
    }
    doc.note_parent(id);
    let rect = doc.pixel_grid().snap_rect(rect);
    out.insert(id, rect);
    let watermark = doc.arena.changed_len();
    doc.deliver_unmeasured_constraint(id, rect.size());
    doc.deliver_placement(id, rect);
    doc.assert_confined(id, watermark, out);
    if !doc.arena.contains(id) {
        return;
    }
    let mut element = doc.arena.take(id);
    let outer = doc.enter_layout(id);
    element.layout(doc, painter, rect, out);
    doc.leave_layout(outer);
    doc.arena.put_back(id, element);
    let settled = doc.arena.changed_len();
    doc.settle_effects();
    doc.assert_confined(id, settled, out);
}
