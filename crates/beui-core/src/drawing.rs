use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use crate::damage::Region;

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
    pub fn erased(draw: impl Any) -> Self {
        Self {
            shared: Rc::new(Shared {
                draw: Box::new(draw),
                latest: Cell::new(0),
                redraws: RefCell::new(VecDeque::new()),
            }),
            revision: 0,
        }
    }

    pub fn draw<T: 'static>(&self) -> Option<&T> {
        self.shared.draw.downcast_ref::<T>()
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

    pub fn same_draw(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shared, &other.shared)
    }

    pub fn damage_since(&self, older: &Self) -> Option<Region> {
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
