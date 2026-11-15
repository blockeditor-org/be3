//! Drop sites and the carry that chooses among their places: a lifted item is offered to
//! every site that accepts it, each gives the places it could land (a `DragSpec`), and each
//! point chooses the closest of them all; a release commits the chosen place to its site.
//! Nothing is written before the release: a site draws what is chosen from its `over`.

use std::any::Any;
use std::rc::Rc;

use beui_view::drag::{
    CandidateId, Carry, CarryId, DragFrame, DragPoint, DragSpec, Evaluation, STICKY, evaluate,
};
use beui_view::reactive::{Get, NodeId, Set, now};
use beui_view::types::Velocity;
use beui_view::types::input::PointerId;

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

/// The carry a lift starts: its payload, the places each accepting site gave at the lift,
/// the last frame and how fast the pointer moves.
pub(crate) struct Placing {
    pub(crate) carry: Carry,
    /// Each accepting site's places, kept apart so a site that goes takes its own with it.
    specs: Vec<(u64, DragSpec<Placed>)>,
    /// The closest of them all, evaluated at each point.
    spec: DragSpec<Placed>,
    frame: DragFrame,
    chosen: Option<Placed>,
    /// The site over and the place chosen in it, last told: a site hears again only when
    /// that changes.
    told: Option<(u64, Option<CandidateId>)>,
    velocity: Velocity,
    last: Option<DragPoint>,
    /// The pointer that lifted it: only it moves and releases the carry.
    pointer: PointerId,
}

fn closest_of(specs: &[(u64, DragSpec<Placed>)]) -> DragSpec<Placed> {
    DragSpec::Closest {
        of: specs.iter().map(|(_, spec)| spec.clone()).collect(),
        sticky: STICKY,
    }
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

    /// Unregisters a site. Mid-carry its places leave the carry, which lets go of what it
    /// chose and chooses again where the pointer last was, so a release never commits to a
    /// site that is gone.
    pub(crate) fn remove_site(&self, id: u64) {
        self.sites.borrow_mut().retain(|site| site.id != id);
        let again = {
            let mut placing = self.placing.borrow_mut();
            match placing.as_mut() {
                Some(placing) if placing.specs.iter().any(|(site, _)| *site == id) => {
                    placing.specs.retain(|(site, _)| *site != id);
                    placing.spec = closest_of(&placing.specs);
                    placing.frame = DragFrame {
                        chosen: None,
                        ..placing.frame
                    };
                    placing.chosen = None;
                    if placing.told.is_some_and(|(site, _)| site == id) {
                        placing.told = None;
                    }
                    placing.last
                }
                _ => None,
            }
        };
        if let Some(last) = again {
            self.place(last, false);
        }
    }

    /// Lifts `carry` at `point`: every site that accepts its payload gives its places, read
    /// from the layout now, and the carry chooses among them from here on.
    pub fn lift(&self, carry: Carry, point: DragPoint) -> Option<CarryId> {
        if self.carrying() {
            return None;
        }
        let id = CarryId(self.next.get());
        self.next.set(id.0 + 1);
        let payload = Rc::clone(&carry.payload);
        let specs: Vec<(u64, DragSpec<Placed>)> = self
            .sites
            .borrow()
            .iter()
            .filter(|site| (site.accepts)(payload.as_ref()))
            .map(|site| {
                let id = site.id;
                let spec =
                    (site.spec)(payload.as_ref()).map(Rc::new(move |drop: Erased| (id, drop)));
                (id, spec)
            })
            .collect();
        *self.placing.borrow_mut() = Some(Placing {
            carry,
            spec: closest_of(&specs),
            specs,
            frame: DragFrame::default(),
            chosen: None,
            told: None,
            pointer: point.pointer,
            velocity: Velocity::new(now(), point.pos),
            last: None,
        });
        self.lifted.update(|lifted| *lifted = Some(payload));
        self.current.set(Some(id));
        self.place(point, false);
        let over = self.over_chosen();
        let places = self.rests().len();
        self.say(match over {
            Some(over) => format!("Lifted, {over}"),
            None => format!("Lifted, {places} places to drop it"),
        });
        Some(id)
    }

    /// Every fixed place of the carry, where it is now.
    fn rests(&self) -> Vec<beui_view::drag::Rest> {
        let space = Rc::clone(&self.space);
        let placed = move |node: NodeId| space.placed(node);
        self.placing
            .borrow()
            .as_ref()
            .map(|placing| beui_view::drag::rests(&placing.spec, &placed))
            .unwrap_or_default()
    }

    fn say(&self, said: String) {
        if let Some(id) = self.current.get_untracked() {
            self.said.set(Some((id, said)));
        }
    }

    /// Moves a carry the keyboard holds to the nearest place `toward` from the one it is
    /// over, aiming at the place's zone when it has one, and says the place then chosen.
    pub fn step(&self, toward: beui_view::types::Vec2) -> bool {
        let Some((item, chosen, drawn, last)) =
            self.placing.borrow().as_ref().and_then(|placing| {
                (placing.pointer == PointerId::Keyboard).then(|| {
                    (
                        placing.carry.item,
                        placing.frame.chosen,
                        placing.frame.item.center(),
                        placing.last,
                    )
                })
            })
        else {
            return false;
        };
        let Some(last) = last else {
            return false;
        };
        let rests = self.rests();
        let from = rests
            .iter()
            .find(|rest| Some(rest.id) == chosen)
            .map_or(drawn, |rest| rest.aim());
        let aims: Vec<_> = rests
            .iter()
            .map(|rest| beui_view::types::Rect::from_center_size(rest.aim(), rest.rest.size()))
            .collect();
        let Some(at) = beui_view::drag::step_toward(&aims, from, toward) else {
            return false;
        };
        let target = &rests[at];
        // A zoned place is chosen while the pointer is in its zone; any other while the item
        // is drawn on its rest.
        let pos = match target.zone {
            Some(zone) => zone.center(),
            None => {
                let item = item.unwrap_or(beui_view::types::Rect::from_center_size(
                    beui_view::types::Pos2::ZERO,
                    target.rest.size(),
                ));
                target.rest.center() - item.center().to_vec2()
            }
        };
        self.place(DragPoint { pos, ..last }, false);
        let said = match self.over_chosen() {
            Some(over) => format!("{}{}", over[..1].to_uppercase(), &over[1..]),
            None => "Over no place".to_owned(),
        };
        self.say(said);
        true
    }

    /// "over the left basket, place 1 of 3": the place the carry has chosen, for a screen
    /// reader; "over a place" for one that is not fixed (free or a nested site), none when it
    /// chose nothing.
    fn over_chosen(&self) -> Option<String> {
        let chosen = self.placing.borrow().as_ref()?.frame.chosen?;
        let rests = self.rests();
        let Some(at) = rests.iter().position(|rest| rest.id == chosen) else {
            return Some("over a place".to_owned());
        };
        let name = rests[at]
            .label
            .clone()
            .unwrap_or_else(|| "a place".to_owned());
        Some(format!("over {name}, place {} of {}", at + 1, rests.len()))
    }

    /// Whether the carry is held by `pointer`; another pointer's moves and releases leave it.
    pub(crate) fn carried_by(&self, pointer: PointerId) -> bool {
        self.placing
            .borrow()
            .as_ref()
            .is_some_and(|placing| placing.pointer == pointer)
    }

    pub(crate) fn placing(&self) -> bool {
        self.placing.borrow().is_some()
    }

    /// The payload carried, if any: tracked, so a site or row reading it follows the lift.
    pub fn carried(&self) -> Option<Erased> {
        self.lifted.get()
    }

    /// The frame the carry shows: where its item draws and whether it floats; tracked. It is
    /// also what peers are shown: a host publishes it as presence from an effect over it
    /// (peers drawing a remote carry wait for plans/declarative_dragging.md, step 5).
    pub fn frame(&self) -> Option<DragFrame> {
        self.shown.get()
    }

    /// Chooses a place for `point`. Only a change is told: the site it left hears it is over
    /// nothing, and the site it is over hears the place chosen in it when that changes.
    pub(crate) fn place(&self, point: DragPoint, released: bool) {
        let space = Rc::clone(&self.space);
        let placed = move |node: NodeId| space.placed(node);
        let (left, entered, frame) = {
            let mut placing = self.placing.borrow_mut();
            let Some(placing) = placing.as_mut() else {
                return;
            };
            let point = DragPoint {
                item: placing.carry.item.unwrap_or(point.item),
                ..point
            };
            // A keyboard carry jumps from place to place: it is never thrown.
            if placing.pointer != PointerId::Keyboard {
                placing.velocity.sample(now(), point.pos);
            }
            placing.last = Some(point);
            let evaluated = evaluate(
                &placing.spec,
                &Evaluation {
                    point,
                    velocity: match placing.pointer {
                        PointerId::Keyboard => beui_view::types::Vec2::ZERO,
                        _ => placing.velocity.get(),
                    },
                    released,
                    placed: &placed,
                },
                &placing.frame,
            );
            placing.frame = evaluated.frame;
            placing.chosen = evaluated.drop;
            let now = placing
                .chosen
                .as_ref()
                .map(|(site, _)| (*site, placing.frame.chosen));
            let was = std::mem::replace(&mut placing.told, now);
            let left = was
                .map(|(site, _)| site)
                .filter(|site| now.map(|(now, _)| now) != Some(*site));
            let entered = (now != was).then(|| placing.chosen.clone()).flatten();
            (left, entered, placing.frame)
        };
        if self.shown.get_untracked() != Some(frame) {
            self.shown.set(Some(frame));
        }
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
        let last = self
            .placing
            .borrow()
            .as_ref()
            .and_then(|placing| placing.last);
        // A pointer's release places once more, as a release (a throw lands further on); a
        // keyboard drop commits the place its last step chose, as said.
        if commit
            && !self.carried_by(PointerId::Keyboard)
            && let Some(last) = last
        {
            self.place(last, true);
        }
        let committing = commit
            && self
                .placing
                .borrow()
                .as_ref()
                .is_some_and(|placing| placing.chosen.is_some());
        self.say(match committing {
            true => "Dropped".to_owned(),
            false => "Put down where it was".to_owned(),
        });
        let Some(placing) = self.placing.borrow_mut().take() else {
            return false;
        };
        self.lifted.update(|lifted| *lifted = None);
        self.shown.set(None);
        self.current.set(None);
        let sites: Vec<(u64, Rc<dyn Fn(Option<Erased>)>, Rc<dyn Fn(Erased, Erased)>)> = self
            .sites
            .borrow()
            .iter()
            .map(|site| (site.id, Rc::clone(&site.over), Rc::clone(&site.commit)))
            .collect();
        let over_site = placing.told.map(|(site, _)| site);
        for (id, over, commit_to) in sites {
            if Some(id) == over_site {
                over(None);
                if commit && let Some((_, drop)) = &placing.chosen {
                    commit_to(Rc::clone(&placing.carry.payload), Rc::clone(drop));
                }
            }
        }
        true
    }
}
