use std::collections::{HashMap, VecDeque};

#[cfg(target_arch = "wasm32")]
use block_plugin_api::SurfaceRect;
use block_plugin_api::ViewRef;
use geometry::{Region, Vec2};

#[cfg(target_arch = "wasm32")]
use crate::PaintTarget;
use crate::ShownView;

#[cfg(target_arch = "wasm32")]
const REMEMBERED_PAINTS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Damage {
    Region(Region),
    Everything,
}

impl Damage {
    pub fn union(self, other: Option<Self>) -> Self {
        match (self, other) {
            (Self::Region(region), Some(Self::Region(added))) => Self::Region(region.union(added)),
            (damage, None) => damage,
            _ => Self::Everything,
        }
    }
}

pub struct Surface<P> {
    pub size: (u32, u32),
    pub age: u32,
    pub visible: Vec2,
    pub painter: P,
    pending: Option<Damage>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    history: VecDeque<Option<Damage>>,
}

impl<P: Default> Default for Surface<P> {
    fn default() -> Self {
        Self {
            size: (0, 0),
            age: 0,
            visible: Vec2::ZERO,
            painter: P::default(),
            pending: None,
            history: VecDeque::new(),
        }
    }
}

impl<P> Surface<P> {
    pub fn damage(&mut self, damage: Damage) {
        self.pending = Some(damage.union(self.pending));
    }

    pub fn unpainted(&self) -> bool {
        self.pending.is_some()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn settle(&mut self) {
        self.pending = None;
    }

    #[cfg(target_arch = "wasm32")]
    pub fn settle(&mut self) {}

    fn keep(&mut self, region: &ShownView) {
        self.size = (region.pixels[0], region.pixels[1]);
        self.age = region.age;
        self.visible = region.visible;
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn paint(
        &mut self,
        target: &PaintTarget<'_>,
        paint: impl FnOnce(&mut P, &PaintTarget<'_>, (u32, u32), Damage) -> Option<Vec<SurfaceRect>>,
    ) -> Vec<SurfaceRect> {
        self.age = target.age;
        let painted = self.pending.take();
        let drawn = self
            .repaint(painted)
            .and_then(|repaint| paint(&mut self.painter, target, self.size, repaint));
        self.history.push_front(drawn.as_ref().and(painted));
        self.history.truncate(REMEMBERED_PAINTS);
        drawn.unwrap_or_default()
    }

    #[cfg(target_arch = "wasm32")]
    fn repaint(&self, painted: Option<Damage>) -> Option<Damage> {
        let older = self
            .age
            .checked_sub(1)
            .map(|older| older as usize)
            .filter(|older| *older <= self.history.len());
        match older {
            Some(older) => self
                .history
                .iter()
                .take(older)
                .fold(painted, |repaint, damage| match (repaint, damage) {
                    (None, damage) => *damage,
                    (Some(repaint), damage) => Some(repaint.union(*damage)),
                }),
            None => Some(Damage::Everything),
        }
    }
}

pub struct Surfaces<P> {
    held: HashMap<ViewRef, Surface<P>>,
}

impl<P> Default for Surfaces<P> {
    fn default() -> Self {
        Self {
            held: HashMap::new(),
        }
    }
}

impl<P: Default> Surfaces<P> {
    pub fn get(&self, region: ViewRef) -> Option<&Surface<P>> {
        self.held.get(&region)
    }

    pub fn get_mut(&mut self, region: ViewRef) -> Option<&mut Surface<P>> {
        self.held.get_mut(&region)
    }

    pub fn of(&mut self, region: ViewRef) -> &mut Surface<P> {
        self.held.entry(region).or_default()
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(ViewRef) -> bool) {
        self.held.retain(|id, _| keep(*id));
    }

    pub(super) fn keep(&mut self, region: &ShownView) {
        self.of(region.id()).keep(region);
    }
}
