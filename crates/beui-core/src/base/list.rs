use std::any::Any;

use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::Painter;
use crate::pixel_grid::PixelGrid;

use crate::base::child_list::{ChildHost, ChildItem, ChildList, NodeChildren};
use crate::base::share::{Part, share, snapped_run};
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    pub fn axes(self, main: f32, cross: f32) -> Vec2 {
        match self {
            Direction::Horizontal => vec2(main, cross),
            Direction::Vertical => vec2(cross, main),
        }
    }

    pub fn main(self, size: Vec2) -> f32 {
        self.main_and_cross(size).0
    }

    pub fn main_and_cross(self, size: Vec2) -> (f32, f32) {
        match self {
            Direction::Horizontal => (size.x, size.y),
            Direction::Vertical => (size.y, size.x),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ItemSize {
    #[default]
    Intrinsic,
    Fixed(f32),
    Percent(f32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sizing {
    pub size: ItemSize,
    pub min: f32,
    pub max: f32,
    pub shrink: f32,
    pub align: Option<Align>,
    pub gap: Option<f32>,
}

impl Default for Sizing {
    fn default() -> Self {
        Self {
            size: ItemSize::Intrinsic,
            min: 0.0,
            max: f32::INFINITY,
            shrink: 0.0,
            align: None,
            gap: None,
        }
    }
}

impl From<ItemSize> for Sizing {
    fn from(size: ItemSize) -> Self {
        Self {
            size,
            ..Self::default()
        }
    }
}

impl Sizing {
    pub fn min(self, min: f32) -> Self {
        Self { min, ..self }
    }

    pub fn max(self, max: f32) -> Self {
        Self { max, ..self }
    }

    pub fn shrink(self, shrink: f32) -> Self {
        Self { shrink, ..self }
    }

    pub fn align(self, align: Align) -> Self {
        Self {
            align: Some(align),
            ..self
        }
    }

    pub fn gap(self, gap: f32) -> Self {
        Self {
            gap: Some(gap),
            ..self
        }
    }

    fn clamp(self, length: f32) -> f32 {
        length.min(self.max).max(self.min)
    }
}

impl ItemSize {
    pub fn min(self, min: f32) -> Sizing {
        Sizing::from(self).min(min)
    }

    pub fn max(self, max: f32) -> Sizing {
        Sizing::from(self).max(max)
    }

    pub fn shrink(self, shrink: f32) -> Sizing {
        Sizing::from(self).shrink(shrink)
    }

    pub fn align(self, align: Align) -> Sizing {
        Sizing::from(self).align(align)
    }

    pub fn gap(self, gap: f32) -> Sizing {
        Sizing::from(self).gap(gap)
    }
}

#[derive(Clone, Copy)]
pub struct ListItem {
    pub child: NodeId,
    pub size: Sizing,
}

impl ChildItem for ListItem {
    fn node(&self) -> NodeId {
        self.child
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
        let sizes: Vec<Sizing> = self.items.iter().map(|item| item.size).collect();
        let mut intrinsic_lengths = Vec::with_capacity(sizes.len());
        for item in self.items.iter() {
            let intrinsic = match item.size.size {
                ItemSize::Intrinsic => true,
                ItemSize::Percent(_) => !available_main.is_finite(),
                ItemSize::Fixed(_) => false,
            };
            intrinsic_lengths.push(match intrinsic {
                true => {
                    let available = self.axes(f32::INFINITY, available_cross);
                    let size = crate::layout::measure(doc, painter, item.child, available);
                    self.main_and_cross(size).0
                }
                false => 0.0,
            });
        }
        let gaps = self.gaps(spacing, grid);
        let lengths = distribute(grid, available_main, &gaps, &sizes, &intrinsic_lengths);
        let mut line = Line::default();
        for ((item, main), gap) in self.items.iter().zip(lengths).zip(gaps) {
            let placed = self.placed(doc, painter, item, main, available_cross, gap, placing);
            line.items.push(placed);
        }
        line.finish();
        line
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
        let grid = doc.pixel_grid();
        let spacing = grid.snap(self.spacing);
        let lines = match self.wrap {
            true => self.lines(doc, painter, available_main),
            false => vec![self.line(doc, painter, available_main, available_cross, true)],
        };
        let single = lines.len() == 1 && !self.wrap;
        let mut line_start = 0.0f32;
        for line in &lines {
            let cross = match single {
                true => available_cross,
                false => line.cross,
            };
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
            line_start += line.cross + spacing;
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        for item in self.items.iter() {
            crate::paint::paint(doc, painter, rects, item.child);
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.items.iter().map(ChildItem::node));
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn passes_scroll_anchor(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        match self.direction {
            Direction::Horizontal => "row",
            Direction::Vertical => "column",
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn justified(grid: PixelGrid, justify: Justify, leftover: f32, items: &[Placed]) -> Vec<f32> {
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
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            if index > 0 {
                exact += between;
            }
            exact += item.gap;
            let start = grid.snap(exact);
            exact += item.main;
            start
        })
        .collect()
}

pub fn distribute_main_axis<S: Copy + Into<Sizing>>(
    grid: PixelGrid,
    available_main: f32,
    spacing: f32,
    sizes: &[S],
    intrinsic_lengths: &[f32],
) -> Vec<f32> {
    let sizes: Vec<Sizing> = sizes.iter().map(|size| (*size).into()).collect();
    let gaps: Vec<f32> = sizes
        .iter()
        .enumerate()
        .map(|(index, size)| match index {
            0 => 0.0,
            _ => grid.snap(size.gap.unwrap_or(spacing)),
        })
        .collect();
    distribute(grid, available_main, &gaps, &sizes, intrinsic_lengths)
}

fn distribute(
    grid: PixelGrid,
    available_main: f32,
    gaps: &[f32],
    sizes: &[Sizing],
    intrinsic_lengths: &[f32],
) -> Vec<f32> {
    let basis: Vec<f32> = sizes
        .iter()
        .zip(intrinsic_lengths)
        .map(|(size, length)| {
            let length = match size.size {
                ItemSize::Fixed(fixed) => grid.snap(fixed.max(0.0)),
                ItemSize::Intrinsic => *length,
                ItemSize::Percent(_) if !available_main.is_finite() => *length,
                ItemSize::Percent(_) => return 0.0,
            };
            grid.snap(size.clamp(length))
        })
        .collect();
    if !available_main.is_finite() {
        return basis;
    }
    let percent = |size: &Sizing| matches!(size.size, ItemSize::Percent(_));
    let taken: f32 = gaps.iter().sum::<f32>()
        + sizes
            .iter()
            .zip(&basis)
            .filter(|(size, _)| !percent(size))
            .map(|(_, length)| length)
            .sum::<f32>();
    let remaining = grid.snap(available_main - taken);
    let parts: Vec<Part> = sizes
        .iter()
        .filter(|size| percent(size))
        .map(|size| match size.size {
            ItemSize::Percent(weight) => Part::new(weight, size.min, size.max),
            _ => unreachable!(),
        })
        .collect();
    let least: f32 = parts.iter().map(|part| part.min).sum();
    let shares = snapped_run(grid, &share(remaining.max(0.0), &parts));
    let overflow = least - remaining;
    let cuts = match overflow > 0.0 {
        true => {
            let parts: Vec<Part> = sizes
                .iter()
                .zip(&basis)
                .map(|(size, length)| match percent(size) {
                    true => Part::new(0.0, 0.0, 0.0),
                    false => Part::new(
                        size.shrink * length,
                        0.0,
                        (length - size.min.max(0.0)).max(0.0),
                    ),
                })
                .collect();
            snapped_run(grid, &share(overflow, &parts))
        }
        false => vec![0.0; sizes.len()],
    };
    let mut shares = shares.into_iter();
    sizes
        .iter()
        .zip(basis)
        .zip(cuts)
        .map(|((size, length), cut)| match percent(size) {
            true => shares.next().unwrap_or(0.0),
            false => length - cut,
        })
        .collect()
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
