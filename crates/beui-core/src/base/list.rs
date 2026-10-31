use std::any::Any;

use crate::geometry::{Rect, Vec2, pos2};
use crate::painter::Painter;
use crate::pixel_grid::PixelGrid;

use crate::child_list::{ChildHost, ChildItem, ChildList, NodeChildren};
use crate::document::Document;
use crate::layout::share::{Part, share, snapped_run};
use crate::node::{Element, NodeId, NodeOf, Rects};

pub use beui_types::layout::{Align, Direction, ItemSize, Justify, Sizing};

#[derive(Clone, Copy)]
pub struct ListItem {
    pub child: NodeId,
    pub size: Sizing,
}

impl ChildItem for ListItem {
    fn node(&self) -> Option<NodeId> {
        Some(self.child)
    }
}

impl ChildHost for ListNode {
    type Stored = ListItem;

    fn children(&mut self) -> &mut ChildList<ListItem> {
        &mut self.items
    }
}

pub struct ListNode {
    pub direction: Direction,
    pub spacing: f32,
    pub align: Align,
    pub justify: Justify,
    pub wrap: bool,
    pub items: ChildList<ListItem>,
}

struct Placed {
    child: NodeId,
    main: f32,
    cross: f32,
    gap: f32,
    align: Align,
    baseline: Option<f32>,
}

#[derive(Default)]
struct Line {
    items: Vec<Placed>,
    main: f32,
    cross: f32,
    ascent: f32,
}

impl Line {
    fn finish(&mut self) {
        let mut ascent = 0.0f32;
        let mut descent = 0.0f32;
        let mut cross = 0.0f32;
        for item in &self.items {
            match item.baseline {
                Some(baseline) => {
                    ascent = ascent.max(baseline);
                    descent = descent.max(item.cross - baseline);
                }
                None => cross = cross.max(item.cross),
            }
        }
        self.ascent = ascent;
        self.cross = cross.max(ascent + descent);
        self.main = self.items.iter().map(|item| item.gap + item.main).sum();
    }
}

impl ListNode {
    fn axes(&self, main: f32, cross: f32) -> Vec2 {
        self.direction.axes(main, cross)
    }

    fn main_and_cross(&self, size: Vec2) -> (f32, f32) {
        self.direction.main_and_cross(size)
    }

    fn align_of(&self, sizing: Sizing) -> Align {
        match (sizing.align.unwrap_or(self.align), self.direction) {
            (Align::Baseline, Direction::Vertical) => Align::Start,
            (align, _) => align,
        }
    }

    fn gaps(&self, spacing: f32, grid: PixelGrid) -> Vec<f32> {
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| match index {
                0 => 0.0,
                _ => grid.snap(item.size.gap.unwrap_or(spacing)),
            })
            .collect()
    }

    fn placed(
        &self,
        doc: &mut Document,
        painter: &Painter,
        item: &ListItem,
        main: f32,
        cross: f32,
        gap: f32,
        placing: bool,
    ) -> Placed {
        let available = self.axes(main, cross);
        let align = self.align_of(item.size);
        let size = match placing && align == Align::Stretch {
            true => Vec2::ZERO,
            false => crate::layout::measure(doc, painter, item.child, available),
        };
        let baseline = match align {
            Align::Baseline => crate::layout::baseline(doc, painter, item.child, available),
            _ => None,
        };
        Placed {
            child: item.child,
            main,
            cross: self.main_and_cross(size).1,
            gap,
            align,
            baseline,
        }
    }

    fn line(
        &self,
        doc: &mut Document,
        painter: &Painter,
        available_main: f32,
        available_cross: f32,
        placing: bool,
    ) -> Line {
        let grid = doc.pixel_grid();
        let spacing = grid.snap(self.spacing);
        let gaps = self.gaps(spacing, grid);
        let mut slots = Vec::with_capacity(self.items.len());
        for (item, gap) in self.items.iter().zip(gaps) {
            let intrinsic = match item.size.size {
                ItemSize::Intrinsic => true,
                ItemSize::Percent(_) => !available_main.is_finite(),
                ItemSize::Fixed(_) => false,
            };
            let length = match intrinsic {
                true => {
                    let available = self.axes(f32::INFINITY, available_cross);
                    let size = crate::layout::measure(doc, painter, item.child, available);
                    self.main_and_cross(size).0
                }
                false => 0.0,
            };
            slots.push(Slot {
                size: self.floored(doc, painter, item, available_main),
                gap,
                length,
            });
        }
        distribute(grid, available_main, &mut slots);
        let mut line = Line {
            items: Vec::with_capacity(slots.len()),
            ..Line::default()
        };
        for (item, slot) in self.items.iter().zip(&slots) {
            let placed = self.placed(
                doc,
                painter,
                item,
                slot.length,
                available_cross,
                slot.gap,
                placing,
            );
            line.items.push(placed);
        }
        line.finish();
        line
    }

    fn floored(
        &self,
        doc: &mut Document,
        painter: &Painter,
        item: &ListItem,
        available_main: f32,
    ) -> Sizing {
        let size = item.size;
        if self.direction != Direction::Horizontal || !available_main.is_finite() || !size.yields()
        {
            return size;
        }
        let least = crate::layout::min_width(doc, painter, item.child).min(size.max);
        size.min(size.min.max(least))
    }

    fn lines(&self, doc: &mut Document, painter: &Painter, available_main: f32) -> Vec<Line> {
        let grid = doc.pixel_grid();
        let spacing = grid.snap(self.spacing);
        let gaps = self.gaps(spacing, grid);
        let mut runs: Vec<Vec<(ListItem, f32, f32)>> = Vec::new();
        let mut run: Vec<(ListItem, f32, f32)> = Vec::new();
        let mut taken = 0.0f32;
        for (item, gap) in self.items.iter().zip(gaps) {
            let basis = match item.size.size {
                ItemSize::Fixed(fixed) => grid.snap(fixed.max(0.0)),
                ItemSize::Intrinsic | ItemSize::Percent(_) => {
                    let unbounded = Vec2::splat(f32::INFINITY);
                    let size = crate::layout::measure(doc, painter, item.child, unbounded);
                    let natural = self.main_and_cross(size).0;
                    match natural > available_main {
                        true => {
                            let available = self.axes(available_main, f32::INFINITY);
                            let size = crate::layout::measure(doc, painter, item.child, available);
                            self.main_and_cross(size).0
                        }
                        false => natural,
                    }
                }
            };
            let basis = grid.snap(item.size.clamp(basis));
            if !run.is_empty() && taken + gap + basis > available_main {
                runs.push(std::mem::take(&mut run));
                taken = 0.0;
            }
            let gap = if run.is_empty() { 0.0 } else { gap };
            taken += gap + basis;
            run.push((*item, basis, gap));
        }
        if !run.is_empty() {
            runs.push(run);
        }
        runs.into_iter()
            .map(|run| {
                let used: f32 = run.iter().map(|(_, basis, gap)| basis + gap).sum();
                let parts: Vec<Part> = run
                    .iter()
                    .map(|(item, basis, _)| match item.size.size {
                        ItemSize::Percent(percent) => {
                            Part::new(percent, 0.0, (item.size.max - basis).max(0.0))
                        }
                        _ => Part::new(0.0, 0.0, 0.0),
                    })
                    .collect();
                let leftover = match available_main.is_finite() {
                    true => (available_main - used).max(0.0),
                    false => 0.0,
                };
                let extra = snapped_run(grid, &share(leftover, &parts));
                let mut line = Line::default();
                for ((item, basis, gap), extra) in run.iter().zip(extra) {
                    let placed = self.placed(
                        doc,
                        painter,
                        item,
                        basis + extra,
                        f32::INFINITY,
                        *gap,
                        false,
                    );
                    line.items.push(placed);
                }
                line.finish();
                line
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn place_line(
        &self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &Rects,
        line: &Line,
        line_start: f32,
        cross: f32,
    ) {
        let grid = doc.pixel_grid();
        let (available_main, _) = self.main_and_cross(rect.size());
        let leftover = match available_main.is_finite() {
            true => (available_main - line.main).max(0.0),
            false => 0.0,
        };
        let starts = justified(grid, self.justify, leftover, &line.items);
        for (item, start) in line.items.iter().zip(starts) {
            let (offset, length) = self.cross_place(grid, item, line, cross);
            let offset = self.axes(start, line_start + offset);
            let child_rect = Rect::from_min_size(
                pos2(rect.left() + offset.x, rect.top() + offset.y),
                self.axes(item.main, length),
            );
            crate::layout::layout(doc, painter, item.child, child_rect, out);
        }
    }

    fn cross_place(&self, grid: PixelGrid, item: &Placed, line: &Line, cross: f32) -> (f32, f32) {
        let length = match item.align {
            Align::Stretch => cross,
            _ => item.cross.min(cross),
        };
        let offset = match item.align {
            Align::Start | Align::Stretch => 0.0,
            Align::Center => grid.snap((cross - length) / 2.0),
            Align::End => cross - length,
            Align::Baseline => match item.baseline {
                Some(baseline) => line.ascent - baseline,
                None => 0.0,
            },
        };
        (offset, length)
    }
}

impl Element for ListNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let (available_main, available_cross) = self.main_and_cross(available);
        if self.wrap {
            let spacing = doc.pixel_grid().snap(self.spacing);
            let lines = self.lines(doc, painter, available_main);
            let main = lines.iter().fold(0.0f32, |main, line| main.max(line.main));
            let cross: f32 = lines.iter().map(|line| line.cross).sum();
            let gaps = spacing * lines.len().saturating_sub(1) as f32;
            return self.axes(main.min(available_main), cross + gaps);
        }
        let line = self.line(doc, painter, available_main, available_cross, false);
        self.axes(line.main, line.cross)
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        let grid = doc.pixel_grid();
        let gaps = self.gaps(grid.snap(self.spacing), grid);
        let in_line = self.direction == Direction::Horizontal && !self.wrap;
        let mut total = 0.0f32;
        for (item, gap) in self.items.iter().zip(gaps) {
            let size = item.size;
            let length = match size.size {
                ItemSize::Fixed(fixed) if self.direction == Direction::Horizontal => {
                    grid.snap(fixed.max(0.0))
                }
                _ => crate::layout::min_width(doc, painter, item.child),
            };
            let length = match self.direction {
                Direction::Horizontal => size.clamp(length),
                Direction::Vertical => length,
            };
            total = match in_line {
                true => total + gap + length,
                false => total.max(length),
            };
        }
        total
    }

    fn least_length(
        &self,
        doc: &mut Document,
        painter: &Painter,
        direction: Direction,
        cross: f32,
    ) -> Option<f32> {
        if self.wrap {
            return None;
        }
        let along = direction == self.direction;
        if !along && direction == Direction::Vertical {
            return None;
        }
        let grid = doc.pixel_grid();
        let gaps = self.gaps(grid.snap(self.spacing), grid);
        let mut total = 0.0f32;
        for (item, gap) in self.items.iter().zip(gaps) {
            let size = item.size;
            if !along {
                let least =
                    crate::layout::least_length(doc, painter, item.child, direction, f32::INFINITY);
                total = total.max(least);
                continue;
            }
            let length = match size.size {
                ItemSize::Fixed(fixed) => grid.snap(fixed.max(0.0)),
                _ if size.yields() => {
                    let least =
                        crate::layout::least_length(doc, painter, item.child, direction, cross);
                    size.clamp(least)
                }
                _ => {
                    let offered = direction.axes(f32::INFINITY, cross);
                    let natural = crate::layout::measure(doc, painter, item.child, offered);
                    size.clamp(direction.main(natural))
                }
            };
            total += gap + length;
        }
        Some(total)
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        let (available_main, available_cross) = self.main_and_cross(available);
        let grid = doc.pixel_grid();
        let line = match self.wrap {
            true => self
                .lines(doc, painter, available_main)
                .into_iter()
                .next()?,
            false => self.line(doc, painter, available_main, available_cross, false),
        };
        let first = line.items.first()?;
        match self.direction {
            Direction::Vertical => {
                let child = self.axes(first.main, available_cross);
                crate::layout::baseline(doc, painter, first.child, child)
            }
            Direction::Horizontal => {
                if line.items.iter().any(|item| item.baseline.is_some()) {
                    return Some(line.ascent);
                }
                let child = self.axes(first.main, available_cross);
                let baseline = crate::layout::baseline(doc, painter, first.child, child)?;
                let (offset, _) = self.cross_place(grid, first, &line, line.cross);
                Some(offset + baseline)
            }
        }
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let (available_main, available_cross) = self.main_and_cross(rect.size());
        if !self.wrap {
            let line = self.line(doc, painter, available_main, available_cross, true);
            self.place_line(doc, painter, rect, out, &line, 0.0, available_cross);
            return;
        }
        let spacing = doc.pixel_grid().snap(self.spacing);
        let mut line_start = 0.0f32;
        for line in &self.lines(doc, painter, available_main) {
            self.place_line(doc, painter, rect, out, line, line_start, line.cross);
            line_start += line.cross + spacing;
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        for item in self.items.iter() {
            crate::paint::paint(doc, painter, rects, item.child);
        }
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn children_revision(&self) -> Option<u64> {
        Some(self.items.revision())
    }

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        for child in self.items.iter().filter_map(ChildItem::node) {
            visit(child);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        crate::child_list::remove_hosted(self, child);
    }

    fn passes_scroll_anchor(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "list"
    }

    fn detail(&self) -> Option<String> {
        (self.direction == Direction::Horizontal).then(|| "horizontal".to_owned())
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        vec![
            ("direction", format!("{:?}", self.direction)),
            ("spacing", self.spacing.to_string()),
            ("align", format!("{:?}", self.align)),
            ("justify", format!("{:?}", self.justify)),
            ("wrap", self.wrap.to_string()),
        ]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn justified(
    grid: PixelGrid,
    justify: Justify,
    leftover: f32,
    items: &[Placed],
) -> impl Iterator<Item = f32> + '_ {
    let count = items.len() as f32;
    let (lead, between) = match justify {
        Justify::Start => (0.0, 0.0),
        Justify::Center => (leftover / 2.0, 0.0),
        Justify::End => (leftover, 0.0),
        Justify::SpaceBetween if items.len() > 1 => (0.0, leftover / (count - 1.0)),
        Justify::SpaceBetween => (0.0, 0.0),
        Justify::SpaceAround => (leftover / count / 2.0, leftover / count),
        Justify::SpaceEvenly => (leftover / (count + 1.0), leftover / (count + 1.0)),
    };
    let mut exact = lead;
    items.iter().enumerate().map(move |(index, item)| {
        if index > 0 {
            exact += between;
        }
        exact += item.gap;
        let start = grid.snap(exact);
        exact += item.main;
        start
    })
}

pub fn distribute_main_axis<S: Copy + Into<Sizing>>(
    grid: PixelGrid,
    available_main: f32,
    spacing: f32,
    sizes: &[S],
    intrinsic_lengths: &[f32],
) -> Vec<f32> {
    let mut slots: Vec<Slot> = sizes
        .iter()
        .zip(intrinsic_lengths)
        .enumerate()
        .map(|(index, (size, length))| {
            let size: Sizing = (*size).into();
            Slot {
                size,
                gap: match index {
                    0 => 0.0,
                    _ => grid.snap(size.gap.unwrap_or(spacing)),
                },
                length: *length,
            }
        })
        .collect();
    distribute(grid, available_main, &mut slots);
    slots.into_iter().map(|slot| slot.length).collect()
}

#[derive(Clone, Copy)]
struct Slot {
    size: Sizing,
    gap: f32,
    length: f32,
}

fn percent(size: &Sizing) -> bool {
    matches!(size.size, ItemSize::Percent(_))
}

fn distribute(grid: PixelGrid, available_main: f32, slots: &mut [Slot]) {
    for slot in slots.iter_mut() {
        let length = match slot.size.size {
            ItemSize::Fixed(fixed) => grid.snap(fixed.max(0.0)),
            ItemSize::Intrinsic => slot.length,
            ItemSize::Percent(_) if !available_main.is_finite() => slot.length,
            ItemSize::Percent(_) => {
                slot.length = 0.0;
                continue;
            }
        };
        slot.length = grid.snap(slot.size.clamp(length));
    }
    if !available_main.is_finite() {
        return;
    }
    let taken: f32 = slots
        .iter()
        .map(|slot| match percent(&slot.size) {
            true => slot.gap,
            false => slot.gap + slot.length,
        })
        .sum();
    let remaining = grid.snap(available_main - taken);
    let mut least = 0.0f32;
    if slots.iter().any(|slot| percent(&slot.size)) {
        let parts: Vec<Part> = slots
            .iter()
            .filter_map(|slot| match slot.size.size {
                ItemSize::Percent(weight) => Some(Part::new(weight, slot.size.min, slot.size.max)),
                _ => None,
            })
            .collect();
        least = parts.iter().map(|part| part.min).sum();
        let shares = snapped_run(grid, &share(remaining.max(0.0), &parts));
        let mut shares = shares.into_iter();
        for slot in slots.iter_mut().filter(|slot| percent(&slot.size)) {
            slot.length = shares.next().unwrap_or(0.0);
        }
    }
    let overflow = least - remaining;
    if overflow > 0.0 {
        let parts: Vec<Part> = slots
            .iter()
            .map(|slot| match percent(&slot.size) {
                true => Part::new(0.0, 0.0, 0.0),
                false => Part::new(
                    slot.size.shrink * slot.length,
                    0.0,
                    (slot.length - slot.size.min.max(0.0)).max(0.0),
                ),
            })
            .collect();
        let cuts = snapped_run(grid, &share(overflow, &parts));
        for (slot, cut) in slots.iter_mut().zip(cuts) {
            if !percent(&slot.size) {
                slot.length -= cut;
            }
        }
    }
}

impl Document {
    pub fn create_list(&mut self, direction: Direction, spacing: f32) -> NodeOf<ListNode> {
        self.arena.insert(ListNode {
            direction,
            spacing,
            align: Align::Stretch,
            justify: Justify::Start,
            wrap: false,
            items: ChildList::default(),
        })
    }

    pub fn set_list_direction(&mut self, list: NodeOf<ListNode>, direction: Direction) {
        if self.arena.get_as::<ListNode>(list).direction != direction {
            self.arena.get_mut_as::<ListNode>(list).direction = direction;
        }
    }

    pub fn set_list_spacing(&mut self, list: NodeOf<ListNode>, spacing: f32) {
        if self.arena.get_as::<ListNode>(list).spacing != spacing {
            self.arena.get_mut_as::<ListNode>(list).spacing = spacing;
        }
    }

    pub fn set_list_align(&mut self, list: NodeOf<ListNode>, align: Align) {
        if self.arena.get_as::<ListNode>(list).align != align {
            self.arena.get_mut_as::<ListNode>(list).align = align;
        }
    }

    pub fn set_list_justify(&mut self, list: NodeOf<ListNode>, justify: Justify) {
        if self.arena.get_as::<ListNode>(list).justify != justify {
            self.arena.get_mut_as::<ListNode>(list).justify = justify;
        }
    }

    pub fn set_list_wrap(&mut self, list: NodeOf<ListNode>, wrap: bool) {
        if self.arena.get_as::<ListNode>(list).wrap != wrap {
            self.arena.get_mut_as::<ListNode>(list).wrap = wrap;
        }
    }

    pub fn append_child(
        &mut self,
        parent: NodeOf<ListNode>,
        child: NodeId,
        size: impl Into<Sizing>,
    ) {
        let size = size.into();
        self.arena
            .get_mut_as::<ListNode>(parent)
            .items
            .push(ListItem { child, size });
    }

    pub fn remove_child(&mut self, parent: NodeOf<ListNode>, child: NodeId) {
        if !self.arena.get_as::<ListNode>(parent).items.contains(child) {
            return;
        }
        self.arena
            .get_mut_as::<ListNode>(parent)
            .items
            .remove(child);
    }

    pub fn set_child_size(&mut self, parent: NodeOf<ListNode>, child: NodeId, size: Sizing) {
        let current = self
            .arena
            .get_as::<ListNode>(parent)
            .items
            .find(child)
            .map(|item| item.size);
        if current == Some(size) {
            return;
        }
        let Some(item) = self
            .arena
            .get_mut_as::<ListNode>(parent)
            .items
            .find_mut(child)
        else {
            return;
        };
        item.size = size;
    }
}

#[cfg(test)]
mod tests;
