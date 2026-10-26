use crate::geometry::{Pos2, Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{NodeId, Rects};

pub fn measure(doc: &mut Document, painter: &Painter, id: NodeId, available: Vec2) -> Vec2 {
    if !doc.arena.contains(id) {
        return Vec2::ZERO;
    }
    doc.note_offer(id, available);
    if let Some(size) = doc.measured(id, available) {
        doc.note_measured(true);
        return size;
    }
    doc.note_measured(false);
    let watermark = doc.arena.layout_revision;
    let element = doc.arena.take(id);
    let outer = doc.enter_measure(id);
    let outer_offer = doc.enter_offer();
    let measured = element.measure(doc, painter, available);
    let offer_dependent = doc.leave_offer(outer_offer);
    doc.leave_measure(outer);
    doc.arena.put_back(id, element);
    let size = doc.pixel_grid().snap_size(measured);
    doc.remember_measurement(id, available, size, offer_dependent, watermark);
    size
}

pub fn baseline(doc: &mut Document, painter: &Painter, id: NodeId, available: Vec2) -> Option<f32> {
    measure(doc, painter, id, available);
    if !doc.arena.contains(id) {
        return None;
    }
    if let Some(baseline) = doc.measured_baseline(id, available) {
        return baseline;
    }
    let watermark = doc.arena.layout_revision;
    let element = doc.arena.take(id);
    let outer = doc.enter_measure(id);
    let baseline = element.baseline(doc, painter, available);
    doc.leave_measure(outer);
    doc.arena.put_back(id, element);
    let baseline = baseline.map(|baseline| doc.pixel_grid().snap(baseline));
    doc.remember_baseline(id, available, baseline, watermark);
    baseline
}

pub fn min_width(doc: &mut Document, painter: &Painter, id: NodeId) -> f32 {
    if !doc.arena.contains(id) {
        return 0.0;
    }
    if let Some(width) = doc.measured_min_width(id) {
        return width;
    }
    let watermark = doc.arena.layout_revision;
    let element = doc.arena.take(id);
    let outer = doc.enter_measure(id);
    let width = element.min_width(doc, painter);
    doc.leave_measure(outer);
    doc.arena.put_back(id, element);
    let width = doc.pixel_grid().snap_up(width);
    doc.remember_min_width(id, width, watermark);
    width
}

pub fn layout(doc: &mut Document, painter: &Painter, id: NodeId, rect: Rect, out: &Rects) {
    if !doc.arena.contains(id) {
        return;
    }
    let rect = doc.pixel_grid().snap_rect(rect);
    doc.note_placed(id);
    let state = painter.state();
    let own = painter.entered(id, rect);
    if doc.reuse_placement(id, rect, state, own.state(), out) {
        doc.note_placed_work(true);
        return;
    }
    doc.note_placed_work(false);
    let culled = doc.culls(id, rect.size(), own.state());
    doc.record_placement(id, rect, state, own.state(), out, culled);
    if culled {
        return;
    }
    let mut element = doc.arena.take(id);
    let frame = doc.enter_layout(id);
    let outer = painter.ctx().swap_space_read(false);
    let local = Rect::from_min_size(Pos2::ZERO, rect.size());
    element.layout(doc, &own, local, out);
    let reads = painter.ctx().swap_space_read(outer);
    doc.note_space_reads(id, reads, frame.base());
    doc.leave_layout(id, frame, out);
    doc.arena.put_back(id, element);
}
