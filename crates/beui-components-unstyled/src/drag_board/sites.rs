//! Drop sites and the carry that chooses among their places: a lifted item is offered to
//! every site that accepts it, each gives the places it could land (a `DragSpec`), and each
//! point chooses the closest of them all; a release commits the chosen place to its site.
//! Nothing is written before the release: a site draws what is chosen from its `over`.

use std::any::Any;
use std::rc::Rc;

use beui_view::drag::{
    Carry, DragFrame, DragPoint, DragSpec, Evaluation, STICKY, evaluate,
};
use beui_view::reactive::{Get, NodeId, Set, now};
use beui_view::types::Velocity;

use super::Board;

pub(crate) type Erased = Rc<dyn Any>;

/// A place in a site, as the board holds it: the site and what dropping there means.
pub(crate) type Placed = (u64, Erased);

/// A site as the board keeps it, with its types erased.
pub(crate) struct Site {
    pub(crate) id: u64,
    pub(crate) accepts: Rc<dyn Fn(&dyn Any) -> bool>,
    pub(crate) spec: Rc<dyn Fn(&dyn Any) -> DragSpec<Erased>>,
    pub(crate) over: Rc<dyn Fn(Option<Erased>)>,
    pub(crate) commit: Rc<dyn Fn(Erased, Erased)>,
}

/// The carry a lift starts: its payload, the places every accepting site gave at the lift,
/// the last frame and how fast the pointer moves.
pub(crate) struct Placing {
    pub(crate) carry: Carry,
    spec: DragSpec<Placed>,
    frame: DragFrame,
    chosen: Option<Placed>,
    over: Option<u64>,
    velocity: Velocity,
    last: Option<DragPoint>,
}

impl Board {
    /// Registers a site; places are asked of it at each lift it accepts.
    pub(crate) fn add_site(&self, mut site: Site) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        site.id = id;
        self.sites.borrow_mut().push(site);
        id
    }

    pub(crate) fn remove_site(&self, id: u64) {
        self.sites.borrow_mut().retain(|site| site.id != id);
        let mut placing = self.placing.borrow_mut();
        if let Some(placing) = placing.as_mut()
            && placing.over == Some(id)
        {
            placing.over = None;
            placing.chosen = None;
        }
    }

    /// Lifts `carry` at `point`: every site that accepts its payload gives its places, read
    /// from the layout now, and the carry chooses among them from here on.
    pub fn lift(&self, carry: Carry, point: DragPoint) {
        if self.carrying() {
            return;
        }
        let payload = Rc::clone(&carry.payload);
        let specs: Vec<DragSpec<Placed>> = self
            .sites
            .borrow()
            .iter()
            .filter(|site| (site.accepts)(payload.as_ref()))
            .map(|site| {
                let id = site.id;
                (site.spec)(payload.as_ref())
                    .map(Rc::new(move |drop: Erased| (id, drop)))
            })
            .collect();
        *self.placing.borrow_mut() = Some(Placing {
            carry,
            spec: DragSpec::Closest {
                of: specs,
                sticky: STICKY,
            },
            frame: DragFrame::default(),
            chosen: None,
            over: None,
            velocity: Velocity::new(now(), point.pos),
            last: None,
        });
        self.lifted.update(|lifted| *lifted = Some(payload));
        self.place(point, false);
    }

    pub(crate) fn placing(&self) -> bool {
        self.placing.borrow().is_some()
    }

    /// The payload carried, if any: tracked, so a site or row reading it follows the lift.
    pub fn carried(&self) -> Option<Erased> {
        self.lifted.get()
    }

    /// The frame the carry shows: where its item draws and whether it floats; tracked.
    pub fn frame(&self) -> Option<DragFrame> {
        self.shown.get()
    }

    /// Chooses a place for `point`, telling the site it is over and the one it left.
    pub(crate) fn place(&self, point: DragPoint, released: bool) {
        let space = Rc::clone(&self.space);
        let placed = move |node: NodeId| space.placed(node);
        let (left, entered) = {
            let mut placing = self.placing.borrow_mut();
            let Some(placing) = placing.as_mut() else {
                return;
            };
            let point = DragPoint {
                item: placing.carry.item,
                ..point
            };
            placing.velocity.sample(now(), point.pos);
            placing.last = Some(point);
            let evaluated = evaluate(
                &placing.spec,
                &Evaluation {
                    point,
                    velocity: placing.velocity.get(),
                    released,
                    placed: &placed,
                },
                &placing.frame,
            );
            placing.frame = evaluated.frame;
            placing.chosen = evaluated.drop;
            let site = placing.chosen.as_ref().map(|(site, _)| *site);
            let left = (placing.over != site).then_some(placing.over).flatten();
            placing.over = site;
            (left, placing.chosen.clone())
        };
        let frame = self.placing.borrow().as_ref().map(|placing| placing.frame);
        self.shown.set(frame);
        if let Some(left) = left.and_then(|id| self.site_over(id)) {
            left(None);
        }
        if let Some((site, drop)) = entered
            && let Some(over) = self.site_over(site)
        {
            over(Some(drop));
        }
    }

    fn site_over(&self, id: u64) -> Option<Rc<dyn Fn(Option<Erased>)>> {
        self.sites
            .borrow()
            .iter()
            .find(|site| site.id == id)
            .map(|site| Rc::clone(&site.over))
    }

    /// Ends the carry: on a release with a place chosen, its site commits the drop; either
    /// way every site hears it is over nothing.
    pub(crate) fn put_down(&self, commit: bool) -> bool {
        let last = self.placing.borrow().as_ref().and_then(|placing| placing.last);
        if commit && let Some(last) = last {
            self.place(last, true);
        }
        let Some(placing) = self.placing.borrow_mut().take() else {
            return false;
        };
        self.lifted.update(|lifted| *lifted = None);
        self.shown.set(None);
        let sites: Vec<(u64, Rc<dyn Fn(Option<Erased>)>, Rc<dyn Fn(Erased, Erased)>)> = self
            .sites
            .borrow()
            .iter()
            .map(|site| (site.id, Rc::clone(&site.over), Rc::clone(&site.commit)))
            .collect();
        for (id, over, commit_to) in sites {
            if Some(id) == placing.over {
                over(None);
                if commit && let Some((_, drop)) = &placing.chosen {
                    commit_to(Rc::clone(&placing.carry.payload), Rc::clone(drop));
                }
            }
        }
        true
    }
}
