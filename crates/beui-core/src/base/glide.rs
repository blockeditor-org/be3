use std::any::Any;

use crate::animation::Spring;
use crate::base::list::Direction;
use crate::callback::Callback;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::node::{Element, NodeId, NodeOf, Placed, Rects};
use crate::painter::Painter;

pub const GLIDE_SPRING: Spring = Spring {
    stiffness: 500.0,
    damping: 42.0,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GlideHold {
    pub at: Pos2,
    pub along: Option<Direction>,
}

#[derive(Clone, Copy)]
struct Origin {
    placed: Option<Placed>,
    window: Pos2,
}

pub struct GlideNode {
    child: Option<NodeId>,
    displacement: Vec2,
    velocity: Vec2,
    origin: Option<Origin>,
    hold: Option<GlideHold>,
    on_start: Callback<()>,
}

impl GlideNode {
    fn take_hold(&mut self, doc: &Document, out: &Rects) {
        let Some(hold) = self.hold else {
            return;
        };
        let Some(placed) = doc.laying_out().and_then(|id| out.placed(&id)) else {
            return;
        };
        let moved = out.transform(placed.space).local_pos(hold.at) - placed.rect.min;
        self.displacement = match hold.along {
            Some(Direction::Horizontal) => Vec2::new(moved.x, 0.0),
            Some(Direction::Vertical) => Vec2::new(0.0, moved.y),
            None => moved,
        };
        self.velocity = Vec2::ZERO;
    }

    fn take_origin(&mut self, doc: &mut Document, out: &Rects) {
        let Some(origin) = self.origin.take() else {
            return;
        };
        let Some(id) = doc.laying_out() else {
            return;
        };
        let Some(placed) = out.placed(&id) else {
            return;
        };
        let moved = match origin.placed {
            Some(from) if from.space == placed.space => from.rect.min - placed.rect.min,
            _ => {
                let transform = out.transform(placed.space);
                transform.local_pos(origin.window) - placed.rect.min
            }
        };
        if moved == Vec2::ZERO || !doc.motion().animates() {
            return;
        }
        self.displacement += moved;
        let on_start = self.on_start;
        if !on_start.is_empty() {
            doc.report_after_layout(move || on_start.call(()));
        }
    }
}

impl Element for GlideNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.child.map_or(Vec2::ZERO, |child| {
            crate::layout::measure(doc, painter, child, available)
        })
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.child
            .map_or(0.0, |child| crate::layout::min_width(doc, painter, child))
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        self.take_origin(doc, out);
        self.take_hold(doc, out);
        let rect = rect.translate(doc.pixel_grid().snap_vec(self.displacement));
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        let Some(child) = self.child else {
            return;
        };
        if self.displacement != Vec2::ZERO
            && rects
                .placed(&child)
                .is_some_and(|placed| !placed.rect.intersects(painter.clip_rect()))
        {
            return;
        }
        crate::paint::paint(doc, painter, rects, child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn kind(&self) -> &'static str {
        "glide"
    }

    fn detail(&self) -> Option<String> {
        (self.displacement != Vec2::ZERO)
            .then(|| format!("{:.1}, {:.1}", self.displacement.x, self.displacement.y))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_glide(&mut self, child: NodeId) -> NodeOf<GlideNode> {
        self.arena.insert(GlideNode {
            child: Some(child),
            displacement: Vec2::ZERO,
            velocity: Vec2::ZERO,
            origin: None,
            hold: None,
            on_start: Callback::empty(),
        })
    }

    pub fn set_glide_on_start(
        &mut self,
        glide: NodeOf<GlideNode>,
        handler: impl FnMut(()) + 'static,
    ) {
        self.arena.touch_mut_as::<GlideNode>(glide).on_start = Callback::new(handler);
    }

    pub fn arm_glide(&mut self, glide: NodeOf<GlideNode>) {
        let Some(placed) = self.rects.placed(&glide.id()) else {
            return;
        };
        let window = self.rects.transform(placed.space).pos(placed.rect.min);
        self.arena.get_mut_as::<GlideNode>(glide).origin = Some(Origin {
            placed: Some(placed),
            window,
        });
    }

    pub fn glide_from(&mut self, glide: NodeOf<GlideNode>, window: Pos2) {
        self.arena.get_mut_as::<GlideNode>(glide).origin = Some(Origin {
            placed: None,
            window,
        });
    }

    pub fn hold_glide(&mut self, glide: NodeOf<GlideNode>, hold: Option<GlideHold>) {
        if !self.arena.contains(glide.id()) {
            return;
        }
        let node = self.arena.get_as::<GlideNode>(glide);
        if hold.is_none() && node.hold.is_none() {
            return;
        }
        let animates = self.motion().animates();
        let node = self.arena.relayout_alone_mut_as::<GlideNode>(glide);
        let released = node.hold.is_some() && hold.is_none();
        node.hold = hold;
        if !released {
            return;
        }
        if !animates {
            node.displacement = Vec2::ZERO;
            return;
        }
        let on_start = node.on_start;
        if node.displacement != Vec2::ZERO && !on_start.is_empty() {
            match self.laying_out() {
                Some(_) => self.report_after_layout(move || on_start.call(())),
                None => crate::current::enter(self, || on_start.call(())),
            }
        }
    }

    pub fn glide_held(&self, glide: NodeOf<GlideNode>) -> bool {
        self.arena.contains(glide.id()) && self.arena.get_as::<GlideNode>(glide).hold.is_some()
    }

    pub fn glide_displacement(&self, glide: NodeOf<GlideNode>) -> Vec2 {
        self.arena.get_as::<GlideNode>(glide).displacement
    }

    pub fn step_glide(&mut self, glide: NodeOf<GlideNode>, seconds: f32) -> bool {
        if !self.arena.contains(glide.id()) {
            return false;
        }
        let node = self.arena.get_as::<GlideNode>(glide);
        if node.hold.is_some() {
            return false;
        }
        let (mut displacement, mut velocity) = (node.displacement, node.velocity);
        GLIDE_SPRING.advance(&mut displacement.x, &mut velocity.x, seconds);
        GLIDE_SPRING.advance(&mut displacement.y, &mut velocity.y, seconds);
        if displacement != node.displacement || velocity != node.velocity {
            let node = self.arena.relayout_alone_mut_as::<GlideNode>(glide);
            node.displacement = displacement;
            node.velocity = velocity;
        }
        displacement != Vec2::ZERO || velocity != Vec2::ZERO
    }

    pub fn settle_glide(&mut self, glide: NodeOf<GlideNode>) {
        if !self.arena.contains(glide.id()) {
            return;
        }
        let node = self.arena.get_as::<GlideNode>(glide);
        if node.displacement != Vec2::ZERO || node.velocity != Vec2::ZERO {
            let node = self.arena.relayout_alone_mut_as::<GlideNode>(glide);
            node.displacement = Vec2::ZERO;
            node.velocity = Vec2::ZERO;
        }
    }
}
