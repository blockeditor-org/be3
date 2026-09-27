use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use crate::damage::Region;

#[cfg(feature = "render")]
use crate::geometry::Vec2;

#[cfg(feature = "render")]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawAt {
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub screen: Vec2,
    pub pixels_per_point: f32,
    pub format: wgpu::TextureFormat,
}

#[cfg(feature = "render")]
impl DrawAt {
    pub fn width(&self) -> u32 {
        (self.rect[2] - self.rect[0]).max(0.0) as u32
    }

    pub fn height(&self) -> u32 {
        (self.rect[3] - self.rect[1]).max(0.0) as u32
    }
}

#[cfg(feature = "render")]
pub trait Draw: 'static {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        _at: DrawAt,
    ) {
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, at: DrawAt);
}

const REMEMBERED_REDRAWS: usize = 16;

struct Shared {
    draw: Box<dyn Any>,
    latest: Cell<u64>,
    redraws: RefCell<VecDeque<(u64, Region)>>,
}

#[derive(Clone)]
pub struct Drawing {
    shared: Rc<Shared>,
    revision: u64,
}

impl Drawing {
    #[cfg(feature = "render")]
    pub fn new(draw: impl Draw) -> Self {
        let draw: Rc<dyn Draw> = Rc::new(draw);
        Self {
            shared: Rc::new(Shared {
                draw: Box::new(draw),
                latest: Cell::new(0),
                redraws: RefCell::new(VecDeque::new()),
            }),
            revision: 0,
        }
    }

    #[cfg(feature = "render")]
    pub(crate) fn draw(&self) -> Option<&Rc<dyn Draw>> {
        self.shared.draw.downcast_ref::<Rc<dyn Draw>>()
    }

    pub fn redrawn(&self, damage: Region) -> Self {
        let revision = self.shared.latest.get() + 1;
        self.shared.latest.set(revision);
        let mut redraws = self.shared.redraws.borrow_mut();
        if redraws.len() == REMEMBERED_REDRAWS {
            redraws.pop_front();
        }
        redraws.push_back((revision, damage));
        Self {
            shared: Rc::clone(&self.shared),
            revision,
        }
    }

    pub(crate) fn same_draw(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shared, &other.shared)
    }

    pub(crate) fn damage_since(&self, older: &Self) -> Option<Region> {
        if !self.same_draw(older) || older.revision > self.revision {
            return None;
        }
        let redraws = self.shared.redraws.borrow();
        let mut region = Region::NOTHING;
        let mut found = 0;
        for (revision, damage) in redraws.iter() {
            if *revision > older.revision && *revision <= self.revision {
                region = region.union(*damage);
                found += 1;
            }
        }
        (found == self.revision - older.revision).then_some(region)
    }
}

impl PartialEq for Drawing {
    fn eq(&self, other: &Self) -> bool {
        self.same_draw(other) && self.revision == other.revision
    }
}

impl std::fmt::Debug for Drawing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Drawing")
            .field("at", &Rc::as_ptr(&self.shared).cast::<()>())
            .field("revision", &self.revision)
            .finish()
    }
}
