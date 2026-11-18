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
use beui_view::types::input::PointerId;
use beui_view::types::{Rect, Velocity};

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
    pub(crate) id: CarryId,
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
    pub(crate) pointer: PointerId,
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

    /// Unregisters a site. Mid-carry its places leave each carry it gave them to, which lets
    /// go of what it chose and chooses again where its pointer last was, so a release never
    /// commits to a site that is gone.
    pub(crate) fn remove_site(&self, id: u64) {
        self.sites.borrow_mut().retain(|site| site.id != id);
        let again: Vec<DragPoint> = self
            .placing
            .borrow_mut()
            .iter_mut()
            .filter(|placing| placing.specs.iter().any(|(site, _)| *site == id))
            .filter_map(|placing| {
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
            })
            .collect();
        for last in again {
            self.place(last, false);
        }
    }

    /// Lifts `carry` at `point`: every site that accepts its payload gives its places, read
    /// from the layout now, and the carry chooses among them from here on. A pointer carries
    /// one at a time, so a lift by one that carries already is refused. A site gives places
    /// to one carry at a time: one already giving them to another carry gives none to this
    /// one (two fingers each lifting a row of one list: the second floats over it).
    pub fn lift(&self, carry: Carry, point: DragPoint) -> Option<CarryId> {
        if !self.lifts(point.pointer) {
            return None;
        }
        let id = CarryId(self.next.get());
        self.next.set(id.0 + 1);
        let payload = Rc::clone(&carry.payload);
        self.lifting.set(Some(carry.item.unwrap_or(point.item)));
        let busy: Vec<u64> = self
            .placing
            .borrow()
            .iter()
            .flat_map(|placing| placing.specs.iter().map(|(site, _)| *site))
            .collect();
        let sites: Vec<(u64, Rc<dyn Fn(&dyn Any) -> DragSpec<Erased>>)> = self
            .sites
            .borrow()
            .iter()
            .filter(|site| !busy.contains(&site.id) && (site.accepts)(payload.as_ref()))
            .map(|site| (site.id, Rc::clone(&site.spec)))
            .collect();
        // A site's spec may read the board (`carried_item`), so the sites are not borrowed
        // while it runs.
        let specs: Vec<(u64, DragSpec<Placed>)> = sites
            .into_iter()
            .map(|(site, spec)| {
                let spec = spec(payload.as_ref()).map(Rc::new(move |drop: Erased| (site, drop)));
                (site, spec)
            })
            .collect();
        self.lifting.set(None);
        self.placing.borrow_mut().push(Placing {
            id,
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
        self.lifted.update(|lifted| lifted.push((id, payload)));
        self.place(point, false);
        let over = self.over_chosen(point.pointer);
        let places = self.rests_of(point.pointer).len();
        self.say(
            id,
            match over {
                Some(over) => format!("Lifted, {over}"),
                None => format!("Lifted, {places} places to drop it"),
            },
        );
        Some(id)
    }

    /// Whether `pointer` may lift a carry: it holds none.
    pub(crate) fn lifts(&self, pointer: PointerId) -> bool {
        !self.carried_by(pointer)
    }

    /// How many sites take `payload`: a site that is the only one has nowhere else to let
    /// it go.
    pub(crate) fn takers(&self, payload: &dyn std::any::Any) -> usize {
        self.sites
            .borrow()
            .iter()
            .filter(|site| (site.accepts)(payload))
            .count()
    }

    /// The rect of the item being lifted while the sites give their places, or else of the
    /// carry lifted last, relative to the pointer; untracked.
    pub(crate) fn carried_item(&self) -> Option<Rect> {
        self.lifting.get().or_else(|| {
            let placing = self.placing.borrow();
            let placing = placing.last()?;
            Some(placing.carry.item.unwrap_or(placing.last?.item))
        })
    }

    /// What the inspector draws of a carry (the keyboard's, or else the first lifted): every
    /// fixed place it can land in, where each is now, and the frame it shows (which place is
    /// chosen, where the item draws). None when nothing is carried; untracked.
    pub fn inspect(&self) -> Option<(Vec<beui_view::drag::Rest>, DragFrame)> {
        let (pointer, frame) = {
            let placing = self.placing.borrow();
            let placing = placing
                .iter()
                .find(|placing| placing.pointer == PointerId::Keyboard)
                .or_else(|| placing.first())?;
            (placing.pointer, placing.frame)
        };
        Some((self.rests_of(pointer), frame))
    }

    /// Every fixed place of `pointer`'s carry, where it is now.
    fn rests_of(&self, pointer: PointerId) -> Vec<beui_view::drag::Rest> {
        let space = Rc::clone(&self.space);
        let placed = move |node: NodeId| space.placed(node);
        self.placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == pointer)
            .map(|placing| beui_view::drag::rests(&placing.spec, &placed))
            .unwrap_or_default()
    }

    fn say(&self, id: CarryId, said: String) {
        self.said.set(Some((id, said)));
    }

    /// The id of `pointer`'s carry.
    fn carry_of(&self, pointer: PointerId) -> Option<CarryId> {
        self.placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == pointer)
            .map(|placing| placing.id)
    }

    /// Says where `pointer`'s carry is now, after a step.
    fn say_over(&self, pointer: PointerId) {
        let Some(id) = self.carry_of(pointer) else {
            return;
        };
        let said = match self.over_chosen(pointer) {
            Some(over) => format!("{}{}", over[..1].to_uppercase(), &over[1..]),
            None => "Over no place".to_owned(),
        };
        self.say(id, said);
    }

    /// Moves the carry the keyboard holds to the nearest place `toward` from the one it is
    /// over, aiming at the place's zone when it has one, and says the place then chosen.
    pub fn step(&self, toward: beui_view::types::Vec2) -> bool {
        let Some((item, chosen, drawn, last)) = self
            .placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == PointerId::Keyboard)
            .map(|placing| {
                (
                    placing.carry.item,
                    placing.frame.chosen,
                    placing.frame.item.center(),
                    placing.last,
                )
            })
        else {
            return false;
        };
        let Some(last) = last else {
            return false;
        };
        let rests = self.rests_of(PointerId::Keyboard);
        let from = rests
            .iter()
            .find(|rest| Some(rest.id) == chosen)
            .map_or(drawn, |rest| rest.aim());
        let aims: Vec<_> = rests
            .iter()
            .map(|rest| {
                (
                    beui_view::types::Rect::from_center_size(rest.aim(), rest.rest.size()),
                    rest.edge,
                )
            })
            .collect();
        let Some(at) = beui_view::drag::step_toward(&aims, from, toward) else {
            return self.step_free(toward, last);
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
        self.say_over(PointerId::Keyboard);
        true
    }

    /// Moves the keyboard's carry, with no fixed place `toward`, that way by its `free`
    /// place's step, where it has one to move in (a canvas, a display's arrangement);
    /// `toward`'s length counts the steps (Shift steps ten).
    fn step_free(&self, toward: beui_view::types::Vec2, last: DragPoint) -> bool {
        let Some(step) = self
            .placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == PointerId::Keyboard)
            .and_then(|placing| beui_view::drag::free_step(&placing.spec))
        else {
            return false;
        };
        let step = toward * step;
        self.place(
            DragPoint {
                pos: last.pos + step,
                ..last
            },
            false,
        );
        self.say_over(PointerId::Keyboard);
        true
    }

    /// "over the left basket, place 1 of 3": the place `pointer`'s carry has chosen, for a
    /// screen reader; "over a place" for one that is not fixed (free or a nested site), none
    /// when it chose nothing.
    fn over_chosen(&self, pointer: PointerId) -> Option<String> {
        let chosen = self
            .placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == pointer)?
            .frame
            .chosen?;
        let rests = self.rests_of(pointer);
        let Some(at) = rests.iter().position(|rest| rest.id == chosen) else {
            return Some("over a place".to_owned());
        };
        let name = rests[at]
            .label
            .clone()
            .unwrap_or_else(|| "a place".to_owned());
        Some(format!("over {name}, place {} of {}", at + 1, rests.len()))
    }

    /// Whether `pointer` holds a carry; another pointer's moves and releases leave it.
    pub(crate) fn carried_by(&self, pointer: PointerId) -> bool {
        self.placing
            .borrow()
            .iter()
            .any(|placing| placing.pointer == pointer)
    }

    /// The nodes the carries' places are relative to, whose windows the board notes.
    pub(crate) fn placing_nodes(&self) -> Vec<beui_view::reactive::NodeRef> {
        self.placing
            .borrow()
            .iter()
            .flat_map(|placing| beui_view::drag::nodes(&placing.spec))
            .collect()
    }

    pub(crate) fn placing(&self) -> bool {
        !self.placing.borrow().is_empty()
    }

    /// Where each carry was last placed.
    pub(crate) fn placing_last(&self) -> Vec<DragPoint> {
        self.placing
            .borrow()
            .iter()
            .filter_map(|placing| placing.last)
            .collect()
    }

    /// Whether `pointer`'s carry scrolls the edge scroller holding it (not `scrolls_nothing`).
    pub(crate) fn placing_scrolls(&self, pointer: PointerId) -> bool {
        self.placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == pointer)
            .is_none_or(|placing| placing.carry.scrolls)
    }

    /// Every carried payload that is a `P`, in the order they were lifted: tracked, so a
    /// site or row reading it follows each lift and put-down.
    pub fn carried<P: Clone + 'static>(&self) -> Vec<P> {
        self.lifted.with(|lifted| {
            lifted
                .iter()
                .filter_map(|(_, payload)| payload.downcast_ref::<P>().cloned())
                .collect()
        })
    }

    /// Whether `payload` is carried; tracked.
    pub fn carries<P: PartialEq + 'static>(&self, payload: &P) -> bool {
        self.carry_with(payload).is_some()
    }

    /// The carry whose payload is `payload`; tracked.
    fn carry_with<P: PartialEq + 'static>(&self, payload: &P) -> Option<CarryId> {
        self.lifted.with(|lifted| {
            lifted
                .iter()
                .find(|(_, carried)| carried.downcast_ref::<P>() == Some(payload))
                .map(|(id, _)| *id)
        })
    }

    /// The frame of the carry whose payload is `payload`: where its item draws and whether
    /// it floats; tracked. A component names its own carry by its payload, so two carries
    /// of one type (two rows of lists of one key type, two splitters) are told apart.
    /// Payloads are names: two equal payloads carried at once are one name, the first lifted. Peers
    /// are shown a carry by its site, which publishes the place chosen in it (its `over`) as
    /// presence in its own terms, so their copy of the site draws it in their view (a
    /// canvas's carried shapes at their world place).
    pub fn frame_of<P: PartialEq + 'static>(&self, payload: &P) -> Option<DragFrame> {
        self.frame_by(self.carry_with(payload)?)
    }

    /// The frame carry `id` shows; tracked.
    pub(crate) fn frame_by(&self, id: CarryId) -> Option<DragFrame> {
        self.shown.with(|shown| {
            shown
                .iter()
                .find(|(shown, _)| *shown == id)
                .map(|(_, frame)| *frame)
        })
    }

    /// Chooses a place for `point`. Only a change is told: the site it left hears it is over
    /// nothing, and the site it is over hears the place chosen in it when that changes.
    pub(crate) fn place(&self, point: DragPoint, released: bool) {
        // A carry kept in a window reads a point over another in that window's points.
        let kept = self
            .placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == point.pointer)
            .and_then(|placing| placing.carry.window);
        // One it cannot read there is over no place of it.
        let (point, unread) = match kept {
            Some(window) if window != point.window => {
                match beui_view::reactive::window_point(point.pos, point.window, window) {
                    Some(pos) => (
                        DragPoint {
                            pos,
                            window,
                            ..point
                        },
                        false,
                    ),
                    None => (point, true),
                }
            }
            _ => (point, false),
        };
        let space = Rc::clone(&self.space);
        // A place in another window than the pointer's is not there for it: its rect is in
        // that window's points. A node whose window is not known yet (before the core first
        // tracks the carry) is taken to be in the pointer's.
        let placed = move |node: NodeId| {
            if self
                .known_window(node)
                .is_some_and(|window| window != point.window)
            {
                return None;
            }
            space.placed(node)
        };
        let (id, left, entered, frame) = {
            let mut placing = self.placing.borrow_mut();
            let Some(placing) = placing
                .iter_mut()
                .find(|placing| placing.pointer == point.pointer)
            else {
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
            // Outside every window no place is under the pointer: a release there is the
            // source's to hear (`Carry::outside`).
            let nowhere = DragSpec::Floating;
            let outside = self.outside.get() && !placing.carry.reads_outside;
            let spec = match outside || unread {
                true => &nowhere,
                false => &placing.spec,
            };
            let evaluated = evaluate(
                spec,
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
            let made = evaluated.made;
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
            // A drop worked out from the point is told again each time: it can change while the
            // same place stays chosen (the site hears only a change of value).
            let entered = (now != was || made)
                .then(|| placing.chosen.clone())
                .flatten();
            (placing.id, left, entered, placing.frame)
        };
        if self.frame_by_untracked(id) != Some(frame) {
            self.shown
                .update(|shown| match shown.iter_mut().find(|(at, _)| *at == id) {
                    Some((_, was)) => *was = frame,
                    None => shown.push((id, frame)),
                });
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

    fn frame_by_untracked(&self, id: CarryId) -> Option<DragFrame> {
        self.shown.with_untracked(|shown| {
            shown
                .iter()
                .find(|(shown, _)| *shown == id)
                .map(|(_, frame)| *frame)
        })
    }

    /// Puts down every carry, dropping nothing (Escape); whether there was one.
    pub(crate) fn put_down_all(&self) -> bool {
        let pointers: Vec<PointerId> = self
            .placing
            .borrow()
            .iter()
            .map(|placing| placing.pointer)
            .collect();
        for &pointer in &pointers {
            self.put_down(pointer, false);
        }
        !pointers.is_empty()
    }

    /// Ends `pointer`'s carry: on a release with a place chosen, its site commits the drop;
    /// either way the site it was over hears it is over nothing.
    pub(crate) fn put_down(&self, pointer: PointerId, commit: bool) -> bool {
        let Some(last) = self
            .placing
            .borrow()
            .iter()
            .find(|placing| placing.pointer == pointer)
            .map(|placing| placing.last)
        else {
            return false;
        };
        // A pointer's release places once more, as a release (a throw lands further on); a
        // keyboard drop commits the place its last step chose, as said.
        if commit
            && pointer != PointerId::Keyboard
            && let Some(last) = last
        {
            self.place(last, true);
        }
        let Some(placing) = ({
            let mut placings = self.placing.borrow_mut();
            placings
                .iter()
                .position(|placing| placing.pointer == pointer)
                .map(|at| placings.remove(at))
        }) else {
            return false;
        };
        let committing = commit && placing.chosen.is_some();
        self.say(
            placing.id,
            match committing {
                true => "Dropped".to_owned(),
                false => "Put down where it was".to_owned(),
            },
        );
        self.stop_edge_of(pointer);
        // Let go outside every window, on no place: the source hears where.
        let outside = (commit && placing.chosen.is_none() && self.outside.get())
            .then(|| placing.carry.outside.clone().zip(placing.last))
            .flatten();
        let id = placing.id;
        self.lifted
            .update(|lifted| lifted.retain(|(lifted, _)| *lifted != id));
        self.shown
            .update(|shown| shown.retain(|(shown, _)| *shown != id));
        let over_site = placing.told.map(|(site, _)| site);
        let site = over_site.and_then(|over_site| {
            self.sites
                .borrow()
                .iter()
                .find(|site| site.id == over_site)
                .map(|site| (Rc::clone(&site.over), Rc::clone(&site.commit)))
        });
        // The site commits before it hears it is over nothing, so a site that previews by
        // writing (a canvas's held move) knows on hearing nothing chosen that no drop came.
        if let Some((over, commit_to)) = site {
            if commit && let Some((_, drop)) = &placing.chosen {
                commit_to(Rc::clone(&placing.carry.payload), Rc::clone(drop));
            }
            over(None);
        }
        if let Some((outside, last)) = outside {
            outside(last);
        }
        true
    }
}
