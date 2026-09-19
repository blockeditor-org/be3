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
        doc.note_measured(true);
        return size;
    }
    doc.note_measured(false);
    let watermark = doc.arena.revision;
    let element = doc.arena.take(id);
    let outer = doc.enter_measure(id);
    let measured = element.measure(doc, painter, available);
    doc.leave_measure(outer);
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
    doc.note_placed(id);
    if doc.reusable_placement(id, rect, out) {
        doc.note_placed_work(true);
        return;
    }
    doc.note_placed_work(false);
    doc.record_placement(id, rect, out);
    let watermark = doc.arena.changed_len();
    doc.deliver_unmeasured_constraint(id, rect.size());
    doc.deliver_placement(id, rect);
    doc.assert_confined(id, watermark);
    if !doc.arena.contains(id) {
        return;
    }
    let mut element = doc.arena.take(id);
    let frame = doc.enter_layout(id);
    element.layout(doc, painter, rect, out);
    doc.leave_layout(id, frame, out);
    doc.arena.put_back(id, element);
    let settled = doc.arena.changed_len();
    doc.settle_effects();
    doc.assert_confined(id, settled);
}
