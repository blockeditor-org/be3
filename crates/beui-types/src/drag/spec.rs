//! Drags as the places they can lead to: a spec names where a carried item can end up, and
//! evaluating it for a point gives where the item draws and which place a release takes. Pure:
//! nodes are placed through the function the board hands in, so it is tested without a core.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use be_lib_geometry::{Direction, Pos2, Rect, Side, Vec2};

use super::DragPoint;
use crate::input::Modifiers;
use crate::node::{NodeId, NodeRef};

/// How close, in points, another candidate must come before a `closest` lets go of the one
/// it chose: behaviour, so a default.
pub const STICKY: f32 = 8.0;
/// How far, in points, the item may be from a `when_far`'s near places before its far one
/// takes over.
pub const REACH: f32 = 96.0;
/// How near, in points, a `snap_radius` pulls the item in.
pub const SNAP_RADIUS: f32 = 48.0;
/// The release speed, in points a second, at which a `thrown` drag carries on along its path.
pub const THROW_SPEED: f32 = 600.0;
/// How far, in points, a keyboard step moves an item that has no fixed place that way and
/// a `free` place to move in, unless the place gives its own (`stepped`); Shift steps ten
/// times as far.
pub const FREE_STEP: f32 = 16.0;
/// How long, in seconds, a thrown drag is carried on along its path to choose where it lands.
pub const THROW_SECONDS: f32 = 0.15;

/// A place a carried item can land: what dropping there means (`drop`), where the item would
/// rest, relative to `node`, and where the pointer chooses it, if not by the nearest rest.
#[derive(Clone)]
pub struct Candidate<D> {
    pub drop: D,
    pub rest: Rect,
    pub zone: Option<Rect>,
    /// Within its zone, chosen only while the pointer is nearer this side of the rect than any
    /// other, each measured as a fraction of the rect's width or height (relative to `node`).
    pub nearest: Option<(Side, Rect)>,
    pub node: NodeRef,
    pub label: Option<String>,
}

impl<D> Candidate<D> {
    pub fn new(drop: D, rest: Rect, node: NodeRef) -> Self {
        Self {
            drop,
            rest,
            zone: None,
            nearest: None,
            node,
            label: None,
        }
    }

    /// Chosen in its zone only while the pointer is nearer `side` of `of` than any other, in
    /// proportion to `of`'s size (`Side::nearest`): a pane's edge bands, which split each
    /// corner along its diagonal.
    pub fn nearest_edge(self, side: Side, of: Rect) -> Self {
        Self {
            nearest: Some((side, of)),
            ..self
        }
    }

    /// Chosen while the pointer is inside `zone` (relative to `node`), rather than when its
    /// rest is the nearest.
    pub fn zoned(self, zone: Rect) -> Self {
        Self {
            zone: Some(zone),
            ..self
        }
    }

    /// What a screen reader announces when the place is chosen.
    pub fn labelled(self, label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            ..self
        }
    }
}

/// Where a carried item can end up, as a tree of choices. Build one with the functions below
/// (`closest(slots).when_far(floating())`).
#[derive(Clone)]
pub enum DragSpec<D> {
    /// One place.
    Fixed(Candidate<D>),
    /// The nearest of several, holding on to the one chosen until another is `sticky` nearer.
    Closest { of: Vec<DragSpec<D>>, sticky: f32 },
    /// Anywhere in `area`: the pointer minus the grab, clamped to it while `bounded`; `drop`
    /// names the rect, in `area`'s own coordinates as a `Candidate`'s are (beyond its edges
    /// when unbounded). A keyboard step moves the item `step` points.
    Free {
        area: NodeRef,
        drop: Rc<dyn Fn(Rect) -> D>,
        bounded: bool,
        step: f32,
    },
    /// `near` while the item is within `reach` of where `near` puts it, else `far`, as when
    /// `near` chooses no place.
    WhenFar {
        near: Box<DragSpec<D>>,
        far: Box<DragSpec<D>>,
        reach: f32,
    },
    /// Carried under the pointer, choosing nothing.
    Floating,
    /// `spec`'s place only while the item is within `radius` of it, else floating. `chain`
    /// asks the board to ask for the places again on entering it, without letting go.
    SnapRadius {
        spec: Box<DragSpec<D>>,
        radius: f32,
        chain: bool,
    },
    /// A release faster than `speed` carries on along its path for `seconds` before choosing.
    Thrown {
        spec: Box<DragSpec<D>>,
        speed: f32,
        seconds: f32,
    },
    /// `spec`'s place, with the item drawn under the pointer along `along` and at the place
    /// across it, as a list's row follows the finger while the rows part around it.
    Following {
        spec: Box<DragSpec<D>>,
        along: Direction,
    },
    /// `held`'s places while the pointer holds `modifiers` (Alt keeping a dock's tab
    /// floating), `otherwise`'s while it does not. While held, its places are the only ones:
    /// they take the item over any other site's, however near those are.
    WhenHeld {
        modifiers: Modifiers,
        held: Box<DragSpec<D>>,
        otherwise: Box<DragSpec<D>>,
    },
    /// `spec`'s place, drawn under the pointer at the size of the place, held at the same
    /// fraction of it as the item was grabbed and kept inside `within`: a dock's tab takes the
    /// shape of the pane it would open while the pointer still carries it.
    Shaped {
        spec: Box<DragSpec<D>>,
        within: NodeRef,
    },
    /// `spec`'s places only at points `open` lets through (where nothing covers the site, a
    /// menu or another window over it); elsewhere the item floats.
    /// `rests` still lists a covered place, so a keyboard carry, which has no point under
    /// anything, can step onto it.
    Open {
        spec: Box<DragSpec<D>>,
        open: Rc<dyn Fn(DragPoint) -> bool>,
    },
    /// A nested site: chosen while the pointer is in `region`, which chooses from there on.
    Into { region: NodeRef, drop: D },
    /// A site whose places are a function of the point (a timeline, where a clip lands at the
    /// frame under the pointer): `place` takes the point in `node`'s own coordinates and the
    /// keys held (a block dropped with Ctrl is copied) and gives what dropping there means, or
    /// `None` where nothing is taken. Chosen wherever it answers;
    /// the item floats under the pointer, and the site draws where it would land from `over`.
    Custom {
        node: NodeRef,
        place: Rc<dyn Fn(Pos2, Modifiers) -> Option<D>>,
    },
}

pub fn fixed<D>(candidate: Candidate<D>) -> DragSpec<D> {
    DragSpec::Fixed(candidate)
}

pub fn closest<D>(of: impl IntoIterator<Item = impl Into<DragSpec<D>>>) -> DragSpec<D> {
    DragSpec::Closest {
        of: of.into_iter().map(Into::into).collect(),
        sticky: STICKY,
    }
}

pub fn free<D>(area: NodeRef, drop: impl Fn(Rect) -> D + 'static) -> DragSpec<D> {
    DragSpec::Free {
        area,
        drop: Rc::new(drop),
        bounded: true,
        step: FREE_STEP,
    }
}

pub fn floating<D>() -> DragSpec<D> {
    DragSpec::Floating
}

pub fn into<D>(region: NodeRef, drop: D) -> DragSpec<D> {
    DragSpec::Into { region, drop }
}

pub fn custom<D>(
    node: NodeRef,
    place: impl Fn(Pos2, Modifiers) -> Option<D> + 'static,
) -> DragSpec<D> {
    DragSpec::Custom {
        node,
        place: Rc::new(place),
    }
}

impl<D> From<Candidate<D>> for DragSpec<D> {
    fn from(candidate: Candidate<D>) -> Self {
        DragSpec::Fixed(candidate)
    }
}

impl<D> DragSpec<D> {
    /// `self` while the item is near it, `far` beyond `REACH`.
    pub fn when_far(self, far: DragSpec<D>) -> Self {
        DragSpec::WhenFar {
            near: Box::new(self),
            far: Box::new(far),
            reach: REACH,
        }
    }

    /// `held` instead of `self` while the pointer holds `modifiers`.
    pub fn when_held(self, modifiers: Modifiers, held: DragSpec<D>) -> Self {
        DragSpec::WhenHeld {
            modifiers,
            held: Box::new(held),
            otherwise: Box::new(self),
        }
    }

    /// Only within `SNAP_RADIUS` of this place; floating beyond it.
    pub fn snap_radius(self) -> Self {
        DragSpec::SnapRadius {
            spec: Box::new(self),
            radius: SNAP_RADIUS,
            chain: false,
        }
    }

    /// A release faster than `THROW_SPEED` carries on along its path before choosing.
    pub fn thrown(self) -> Self {
        DragSpec::Thrown {
            spec: Box::new(self),
            speed: THROW_SPEED,
            seconds: THROW_SECONDS,
        }
    }

    /// A `thrown` that carries a release on along its path for `seconds` rather than
    /// `THROW_SECONDS`: a sheet's detents, flicked past from high up.
    pub fn thrown_for(self, seconds: f32) -> Self {
        match self {
            DragSpec::Thrown { spec, speed, .. } => DragSpec::Thrown {
                spec,
                speed,
                seconds,
            },
            other => other,
        }
    }

    /// The place this chooses, with the item drawn under the pointer along `along`.
    pub fn following(self, along: Direction) -> Self {
        DragSpec::Following {
            spec: Box::new(self),
            along,
        }
    }

    /// The place this chooses, with the item drawn under the pointer at the place's size,
    /// kept inside `within`.
    pub fn shaped(self, within: NodeRef) -> Self {
        DragSpec::Shaped {
            spec: Box::new(self),
            within,
        }
    }

    /// These places only at points `open` lets through.
    pub fn open_where(self, open: impl Fn(DragPoint) -> bool + 'static) -> Self {
        DragSpec::Open {
            spec: Box::new(self),
            open: Rc::new(open),
        }
    }

    /// The same spec with another reach, radius, stickiness or throw speed, where it has one.
    pub fn with_distance(self, distance: f32) -> Self {
        match self {
            DragSpec::Closest { of, .. } => DragSpec::Closest {
                of,
                sticky: distance,
            },
            DragSpec::WhenFar { near, far, .. } => DragSpec::WhenFar {
                near,
                far,
                reach: distance,
            },
            DragSpec::SnapRadius { spec, chain, .. } => DragSpec::SnapRadius {
                spec,
                radius: distance,
                chain,
            },
            DragSpec::Thrown { spec, seconds, .. } => DragSpec::Thrown {
                spec,
                speed: distance,
                seconds,
            },
            other => other,
        }
    }

    /// A `free` whose item is not kept inside its area: an infinite canvas, whose area is a
    /// window onto its world that the edge band scrolls.
    pub fn unbounded(self) -> Self {
        match self {
            DragSpec::Free {
                area, drop, step, ..
            } => DragSpec::Free {
                area,
                drop,
                bounded: false,
                step,
            },
            other => other,
        }
    }

    /// A `free` whose keyboard step is `step` points rather than `FREE_STEP`: a canvas steps
    /// a carried selection as its arrows nudge one.
    pub fn stepped(self, step: f32) -> Self {
        match self {
            DragSpec::Free {
                area,
                drop,
                bounded,
                ..
            } => DragSpec::Free {
                area,
                drop,
                bounded,
                step,
            },
            other => other,
        }
    }

    /// A `snap_radius` that chains: entering it asks for the places again.
    pub fn chained(self) -> Self {
        match self {
            DragSpec::SnapRadius { spec, radius, .. } => DragSpec::SnapRadius {
                spec,
                radius,
                chain: true,
            },
            other => other,
        }
    }
}

impl<D: 'static> DragSpec<D> {
    /// The same places with what dropping on each means mapped by `drop`: how a board
    /// erases a site's places, or tags them with the site.
    pub fn map<E: 'static>(self, drop: Rc<dyn Fn(D) -> E>) -> DragSpec<E> {
        match self {
            DragSpec::Fixed(candidate) => DragSpec::Fixed(Candidate {
                drop: drop(candidate.drop),
                rest: candidate.rest,
                zone: candidate.zone,
                nearest: candidate.nearest,
                node: candidate.node,
                label: candidate.label,
            }),
            DragSpec::Closest { of, sticky } => DragSpec::Closest {
                of: of
                    .into_iter()
                    .map(|spec| spec.map(Rc::clone(&drop)))
                    .collect(),
                sticky,
            },
            DragSpec::Free {
                area,
                drop: named,
                bounded,
                step,
            } => DragSpec::Free {
                area,
                drop: Rc::new(move |rect| drop(named(rect))),
                bounded,
                step,
            },
            DragSpec::WhenFar { near, far, reach } => DragSpec::WhenFar {
                near: Box::new(near.map(Rc::clone(&drop))),
                far: Box::new(far.map(drop)),
                reach,
            },
            DragSpec::Floating => DragSpec::Floating,
            DragSpec::WhenHeld {
                modifiers,
                held,
                otherwise,
            } => DragSpec::WhenHeld {
                modifiers,
                held: Box::new(held.map(Rc::clone(&drop))),
                otherwise: Box::new(otherwise.map(drop)),
            },
            DragSpec::SnapRadius {
                spec,
                radius,
                chain,
            } => DragSpec::SnapRadius {
                spec: Box::new(spec.map(drop)),
                radius,
                chain,
            },
            DragSpec::Thrown {
                spec,
                speed,
                seconds,
            } => DragSpec::Thrown {
                spec: Box::new(spec.map(drop)),
                speed,
                seconds,
            },
            DragSpec::Following { spec, along } => DragSpec::Following {
                spec: Box::new(spec.map(drop)),
                along,
            },
            DragSpec::Shaped { spec, within } => DragSpec::Shaped {
                spec: Box::new(spec.map(drop)),
                within,
            },
            DragSpec::Open { spec, open } => DragSpec::Open {
                spec: Box::new(spec.map(drop)),
                open,
            },
            DragSpec::Into { region, drop: into } => DragSpec::Into {
                region,
                drop: drop(into),
            },
            DragSpec::Custom { node, place } => DragSpec::Custom {
                node,
                place: Rc::new(move |point, held| place(point, held).map(|made| drop(made))),
            },
        }
    }
}

/// What a drag lifts: its payload, erased (a board hands it back typed), and the item's rect
/// relative to the pointer, which the item keeps while it floats. With no `item`, an
/// `Interactive` lifting it gives its own node, held where it was pressed. Its `face`, set by
/// `preview` or `drawn_by_site`, says what is drawn for the item.
#[derive(Clone)]
pub struct Carry {
    pub payload: Rc<dyn std::any::Any>,
    pub item: Option<Rect>,
    face: CarryFace,
    pub outside: Option<Rc<dyn Fn(DragPoint)>>,
    /// The window the carry is read in, whatever window the pointer is over (`kept_in`).
    pub window: Option<crate::window::WindowId>,
    /// Whether the carry scrolls the edge scroller whose band holds it (`scrolls_nothing`).
    pub scrolls: bool,
    /// Whether its places are still read with the pointer outside every window
    /// (`reads_outside`).
    pub reads_outside: bool,
}

/// What is drawn for a carried item.
#[derive(Clone, Default)]
pub enum CarryFace {
    /// The source itself, displaced to where the item is and raised above its siblings.
    #[default]
    Source,
    /// What the function builds, floating under the pointer in a layer above the window.
    Preview(Rc<dyn Fn() -> NodeId>),
    /// Nothing from the source: a site draws the item (a `Reorder` draws its own carried
    /// row, which collapses as it leaves).
    Site,
}

impl Carry {
    pub fn new<P: 'static>(payload: P) -> Self {
        Self {
            payload: Rc::new(payload),
            item: None,
            face: CarryFace::Source,
            outside: None,
            window: None,
            scrolls: true,
            reads_outside: false,
        }
    }

    /// Reads its places at the pointer's point in its window wherever the pointer is, also
    /// outside every window, where a carry otherwise chooses no place and a release tells its
    /// `outside`: a splitter, whose `free` place clamps a point beyond the window's edge.
    pub fn reads_outside(self) -> Self {
        Self {
            reads_outside: true,
            ..self
        }
    }

    /// Scrolls nothing while carried, wherever it is held: a control's thumb, whose drag moves
    /// a value, not an item, so a form scrolling under it would move the value too.
    pub fn scrolls_nothing(self) -> Self {
        Self {
            scrolls: false,
            ..self
        }
    }

    /// Reads every point in `window`'s points, where the pointer's window relates to it
    /// (`window_point`), rather than leaving the places of `window` to a pointer over it: an
    /// item that lives in one window and is drawn above that window's own windows (a dock's
    /// tool bar, carried over a window of its dock or out of the window it was popped into).
    pub fn kept_in(self, window: crate::window::WindowId) -> Self {
        Self {
            window: Some(window),
            ..self
        }
    }

    /// Heard when the carry is let go outside every window, where no place is: the source
    /// decides what that means (a dock floats the tab in a window of its own there).
    pub fn outside(self, outside: impl Fn(DragPoint) + 'static) -> Self {
        Self {
            outside: Some(Rc::new(outside)),
            ..self
        }
    }

    /// What floats under the pointer instead of the source: built when the item floats, in
    /// a layer above the window, so no scroll clips it.
    pub fn preview(self, preview: impl Fn() -> NodeId + 'static) -> Self {
        Self {
            face: CarryFace::Preview(Rc::new(preview)),
            ..self
        }
    }

    /// What is drawn for the item.
    pub fn face(&self) -> &CarryFace {
        &self.face
    }

    /// Draws nothing for the item from the source: a site draws it.
    pub fn drawn_by_site(self) -> Self {
        Self {
            face: CarryFace::Site,
            ..self
        }
    }

    /// The item's rect relative to the pointer, as the source drew it when lifted.
    pub fn item(self, item: Rect) -> Self {
        Self {
            item: Some(item),
            ..self
        }
    }
}

/// A carry a board holds, as `Driver::lift` names it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CarryId(pub u64);

/// Which place a frame chose: the position of its spec in the tree, counted depth first, so it
/// stays the same while the spec does.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CandidateId(pub usize);

/// One frame of a drag: where the item draws, in window points, which place a release takes,
/// and whether it floats under the pointer. Also what peers are shown.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DragFrame {
    pub item: Rect,
    pub chosen: Option<CandidateId>,
    pub floating: bool,
    /// Entered a chaining `snap_radius`: ask for the places again.
    pub chain: bool,
}

impl Default for DragFrame {
    fn default() -> Self {
        Self {
            item: Rect::ZERO,
            chosen: None,
            floating: true,
            chain: false,
        }
    }
}

/// A frame and what dropping on its chosen place means.
pub struct Evaluated<D> {
    pub frame: DragFrame,
    pub drop: Option<D>,
    /// Whether `drop` was worked out from the point (`free`, `custom`), so it can change while
    /// the same place stays chosen.
    pub made: bool,
}

/// What evaluating a spec reads: the point, how fast it moves (points a second), whether it
/// is the release, and where a node is placed (its rect and scale, as `Space::placed`).
pub struct Evaluation<'a> {
    pub point: DragPoint,
    pub velocity: Vec2,
    pub released: bool,
    pub placed: &'a dyn Fn(NodeId) -> Option<(Rect, f32)>,
}

impl Evaluation<'_> {
    /// Where the item draws under the pointer.
    fn carried(&self) -> Rect {
        self.point.item.translate(self.point.pos.to_vec2())
    }

    fn place(&self, node: &NodeRef, rect: Rect) -> Option<Rect> {
        let (at, scale) = (self.placed)(node.try_get()?)?;
        Some(Rect::from_min_size(
            at.min + rect.min.to_vec2() * scale,
            rect.size() * scale,
        ))
    }
}

/// The frame `spec` gives for a point, and what dropping there means. `last` is the frame
/// before, which `closest` holds on to.
pub fn evaluate<D: Clone>(
    spec: &DragSpec<D>,
    evaluation: &Evaluation,
    last: &DragFrame,
) -> Evaluated<D> {
    let carried = evaluation.carried();
    let mut next = 0;
    let found = walk(spec, evaluation, last, carried, &mut next, None);
    match found {
        Some(found) => Evaluated {
            frame: DragFrame {
                item: found.item,
                chosen: found.chosen,
                floating: found.floating,
                chain: found.chain,
            },
            made: matches!(found.drop, Some(Dropped::Made(_))),
            drop: found.drop.map(Dropped::owned),
        },
        None => Evaluated {
            frame: DragFrame {
                item: carried,
                chosen: None,
                floating: true,
                chain: false,
            },
            drop: None,
            made: false,
        },
    }
}

/// What dropping on a found place means: borrowed from the spec until a place wins, so only
/// the winner's is cloned.
enum Dropped<'s, D> {
    Spec(&'s D),
    Made(D),
}

impl<D: Clone> Dropped<'_, D> {
    fn owned(self) -> D {
        match self {
            Dropped::Spec(drop) => drop.clone(),
            Dropped::Made(drop) => drop,
        }
    }
}

struct Found<'s, D> {
    item: Rect,
    chosen: Option<CandidateId>,
    drop: Option<Dropped<'s, D>>,
    floating: bool,
    chain: bool,
    /// How far the place is from the carried item: what `closest` and `when_far` compare.
    distance: f32,
    /// Chosen by the pointer being in a zone, not by the nearest rest: held with no
    /// stickiness, since the zone itself says where it starts and ends.
    zoned: bool,
    /// Found on the side of a `when_held` whose modifiers are held: a `closest` takes only
    /// such places while any is found.
    forced: bool,
}

impl<D> Found<'_, D> {
    /// A place was found: anything chosen, not merely floating.
    fn placed(&self) -> bool {
        self.chosen.is_some()
    }
}

fn floated<'s, D>(carried: Rect) -> Found<'s, D> {
    Found {
        item: carried,
        chosen: None,
        drop: None,
        floating: true,
        chain: false,
        distance: 0.0,
        zoned: false,
        forced: false,
    }
}

fn distance(carried: Rect, rest: Rect) -> f32 {
    (carried.center() - rest.center()).length()
}

/// Gives every node of `spec` its id in depth-first order and evaluates it. `None` when no
/// place can be found (a candidate whose node is gone).
fn walk<'s, D: Clone>(
    spec: &'s DragSpec<D>,
    evaluation: &Evaluation,
    last: &DragFrame,
    carried: Rect,
    next: &mut usize,
    along: Option<Direction>,
) -> Option<Found<'s, D>> {
    let id = CandidateId(*next);
    *next += 1;
    match spec {
        DragSpec::Fixed(candidate) => {
            let rest = evaluation.place(&candidate.node, candidate.rest)?;
            let distance = match candidate.zone {
                Some(zone) => {
                    let zone = evaluation.place(&candidate.node, zone)?;
                    let pos = evaluation.point.pos;
                    let nearest = match candidate.nearest {
                        Some((edge, of)) => {
                            let of = evaluation.place(&candidate.node, of)?;
                            Side::nearest(of, pos) == Some(edge)
                        }
                        None => true,
                    };
                    match zone.contains(pos) && nearest {
                        true => 0.0,
                        false => f32::INFINITY,
                    }
                }
                None => match along {
                    Some(along) => along.main(carried.center() - rest.center()).abs(),
                    None => distance(carried, rest),
                },
            };
            Some(Found {
                item: rest,
                chosen: Some(id),
                drop: Some(Dropped::Spec(&candidate.drop)),
                floating: false,
                chain: false,
                distance,
                zoned: candidate.zone.is_some(),
                forced: false,
            })
        }
        DragSpec::Closest { of, sticky } => {
            // The place held from the last frame, the nearest place, and the first floating
            // found: a place always beats floating, so a far site floating never takes the
            // item from a near site's place; it floats only when no place is in reach. A
            // place infinitely far (a zone the pointer left) is let go.
            let mut held: Option<(usize, Found<D>)> = None;
            let mut nearest: Option<(usize, Found<D>)> = None;
            let mut floats: Option<Found<D>> = None;
            let found: Vec<(usize, Found<D>)> = of
                .iter()
                .enumerate()
                .filter_map(|(at, spec)| {
                    Some((at, walk(spec, evaluation, last, carried, next, along)?))
                })
                .collect();
            // A held modifier's places, where any is found, are the only ones.
            let forced = found
                .iter()
                .any(|(_, found)| found.forced && found.placed());
            for (at, found) in found {
                if forced && !(found.forced && found.placed()) {
                    continue;
                }
                if !found.placed() {
                    floats.get_or_insert(found);
                    continue;
                }
                if !found.distance.is_finite() {
                    continue;
                }
                if found.chosen == last.chosen {
                    held = Some((at, found));
                } else if nearest
                    .as_ref()
                    .is_none_or(|(_, nearest)| found.distance < nearest.distance)
                {
                    nearest = Some((at, found));
                }
            }
            match (held, nearest) {
                // A zone held gives way to a nearer place at once, and among places as near
                // (zones that both hold the pointer, one inside the other) to the one listed
                // first: a zone needs no stickiness, as it says itself where it ends.
                (Some((held_at, held)), Some((nearest_at, nearest))) if held.zoned => {
                    let first = nearest.distance < held.distance
                        || (nearest.distance == held.distance && nearest_at < held_at);
                    match first {
                        true => Some(nearest),
                        false => Some(held),
                    }
                }
                (Some((_, held)), Some((_, nearest))) => {
                    match nearest.distance + sticky >= held.distance {
                        true => Some(held),
                        false => Some(nearest),
                    }
                }
                (Some((_, held)), None) => Some(held),
                (None, Some((_, nearest))) => Some(nearest),
                (None, None) => floats,
            }
        }
        DragSpec::Free {
            area,
            drop,
            bounded,
            ..
        } => {
            let (area, scale) = (evaluation.placed)(area.try_get()?)?;
            let item = match bounded {
                true => clamp_into(carried, area),
                false => carried,
            };
            let local = Rect::from_min_size(
                Pos2::ZERO + (item.min - area.min) / scale,
                item.size() / scale,
            );
            Some(Found {
                item,
                chosen: Some(id),
                drop: Some(Dropped::Made(drop(local))),
                floating: false,
                chain: false,
                distance: distance(carried, item),
                zoned: false,
                forced: false,
            })
        }
        DragSpec::WhenFar { near, far, reach } => {
            let near = walk(near, evaluation, last, carried, next, along);
            // Ids stay the same whichever side is chosen, so count `far`'s even when unused.
            let far = walk(far, evaluation, last, carried, next, along);
            // A near side that found no place (its places all closed) is as far as can be.
            match near {
                Some(near) if near.placed() && near.distance <= *reach => Some(near),
                _ => far,
            }
        }
        DragSpec::Floating => Some(floated(carried)),
        DragSpec::WhenHeld {
            modifiers,
            held,
            otherwise,
        } => {
            // Ids stay the same whichever side is chosen, so count both.
            let holds = evaluation.point.modifiers.holds(*modifiers);
            let held = walk(held, evaluation, last, carried, next, along);
            let otherwise = walk(otherwise, evaluation, last, carried, next, along);
            match holds {
                true => held.map(|held| Found {
                    forced: true,
                    ..held
                }),
                false => otherwise,
            }
        }
        DragSpec::SnapRadius {
            spec,
            radius,
            chain,
        } => {
            let found = walk(spec, evaluation, last, carried, next, along)?;
            match found.distance <= *radius {
                true => {
                    let entered = found.chosen.is_some() && found.chosen != last.chosen;
                    Some(Found {
                        chain: *chain && entered,
                        ..found
                    })
                }
                false => Some(floated(carried)),
            }
        }
        DragSpec::Thrown {
            spec,
            speed,
            seconds,
        } => {
            let thrown = evaluation.released && evaluation.velocity.length() >= *speed;
            if !thrown {
                return walk(spec, evaluation, last, carried, next, along);
            }
            let ahead = evaluation.velocity * *seconds;
            let point = DragPoint {
                pos: evaluation.point.pos + ahead,
                ..evaluation.point
            };
            let projected = Evaluation {
                point,
                velocity: evaluation.velocity,
                released: true,
                placed: evaluation.placed,
            };
            walk(
                spec,
                &projected,
                last,
                carried.translate(ahead),
                next,
                along,
            )
        }
        DragSpec::Following { spec, along: axis } => {
            // Its own places are chosen by how far along the line they are, as a list's row
            // passes its neighbours however far across it the pointer strays; against other
            // sites' places it counts the whole distance to the one it chose.
            let found = walk(spec, evaluation, last, carried, next, Some(*axis))?;
            if !found.placed() {
                return Some(found);
            }
            let found = Found {
                distance: match found.distance.is_finite() {
                    true => distance(carried, found.item),
                    false => found.distance,
                },
                ..found
            };
            let item = match axis {
                Direction::Vertical => found
                    .item
                    .translate(Vec2::new(0.0, carried.min.y - found.item.min.y)),
                Direction::Horizontal => found
                    .item
                    .translate(Vec2::new(carried.min.x - found.item.min.x, 0.0)),
            };
            Some(Found { item, ..found })
        }
        DragSpec::Shaped { spec, within } => {
            let found = walk(spec, evaluation, last, carried, next, along)?;
            if !found.placed() || found.floating {
                return Some(found);
            }
            // Held at the fraction of the item it was grabbed at, so the pointer stays on
            // the same part of it whatever its size.
            let grabbed = evaluation.point.item;
            let fraction = |at: f32, length: f32| match length > 0.0 {
                true => (-at / length).clamp(0.0, 1.0),
                false => 0.5,
            };
            let size = found.item.size();
            let held = Vec2::new(
                fraction(grabbed.min.x, grabbed.width()) * size.x,
                fraction(grabbed.min.y, grabbed.height()) * size.y,
            );
            let item = Rect::from_min_size(evaluation.point.pos - held, size);
            let item = match within.try_get().and_then(evaluation.placed) {
                Some((area, _)) => clamp_into(item, area),
                None => item,
            };
            Some(Found { item, ..found })
        }
        DragSpec::Open { spec, open } => {
            // Ids stay the same whether or not the point is open, so walk it either way.
            let found = walk(spec, evaluation, last, carried, next, along);
            match open(evaluation.point) {
                true => found,
                false => found.map(|_| floated(carried)),
            }
        }
        DragSpec::Into { region, drop } => {
            let node = region.try_get()?;
            let (rect, _) = (evaluation.placed)(node)?;
            let inside = rect.contains(evaluation.point.pos);
            Some(Found {
                item: carried,
                chosen: Some(id),
                drop: Some(Dropped::Spec(drop)),
                floating: true,
                chain: false,
                distance: match inside {
                    true => 0.0,
                    false => f32::INFINITY,
                },
                zoned: true,
                forced: false,
            })
        }
        DragSpec::Custom { node, place } => {
            let (at, scale) = (evaluation.placed)(node.try_get()?)?;
            let local = Pos2::ZERO + (evaluation.point.pos - at.min) / scale;
            let drop = place(local, evaluation.point.modifiers)?;
            Some(Found {
                item: carried,
                chosen: Some(id),
                drop: Some(Dropped::Made(drop)),
                floating: true,
                chain: false,
                distance: 0.0,
                zoned: true,
                forced: false,
            })
        }
    }
}

/// A fixed place of a spec, as a keyboard carry steps between them: where the item rests
/// there, the zone the pointer must be in for it to be chosen (if it has one), its label,
/// and the id `evaluate` names it by.
#[derive(Clone, PartialEq, Debug)]
pub struct Rest {
    pub rest: Rect,
    pub zone: Option<Rect>,
    /// The side its zone is chosen nearest (`Candidate::nearest_edge`), if any.
    pub edge: Option<Side>,
    pub label: Option<String>,
    pub id: CandidateId,
}

impl Rest {
    /// Where a pointer chooses this place: the middle of its zone, else of its rest.
    pub fn aim(&self) -> Pos2 {
        self.zone.unwrap_or(self.rest).center()
    }
}

/// Every fixed place in `spec`, where it is now, in the spec's order. A place whose node is
/// gone is left out; ids are counted as `evaluate` counts them.
/// Both sides of a `when_held` are walked, as `evaluate` counts both, so a keyboard carry,
/// which holds no modifier, steps among the places of either.
pub fn rests<D>(spec: &DragSpec<D>, placed: &dyn Fn(NodeId) -> Option<(Rect, f32)>) -> Vec<Rest> {
    fn walk<D>(
        spec: &DragSpec<D>,
        placed: &dyn Fn(NodeId) -> Option<(Rect, f32)>,
        next: &mut usize,
        into: &mut Vec<Rest>,
    ) {
        let id = CandidateId(*next);
        *next += 1;
        match spec {
            DragSpec::Fixed(candidate) => {
                let place = |rect: Rect| {
                    let (at, scale) = candidate.node.try_get().and_then(placed)?;
                    Some(Rect::from_min_size(
                        at.min + rect.min.to_vec2() * scale,
                        rect.size() * scale,
                    ))
                };
                if let Some(rest) = place(candidate.rest) {
                    into.push(Rest {
                        rest,
                        zone: candidate.zone.and_then(place),
                        edge: candidate.nearest.map(|(side, _)| side),
                        label: candidate.label.clone(),
                        id,
                    });
                }
            }
            DragSpec::Closest { of, .. } => {
                of.iter().for_each(|spec| walk(spec, placed, next, into))
            }
            DragSpec::WhenFar { near, far, .. } => {
                walk(near, placed, next, into);
                walk(far, placed, next, into);
            }
            DragSpec::WhenHeld {
                held, otherwise, ..
            } => {
                walk(held, placed, next, into);
                walk(otherwise, placed, next, into);
            }
            DragSpec::SnapRadius { spec, .. }
            | DragSpec::Thrown { spec, .. }
            | DragSpec::Following { spec, .. }
            | DragSpec::Shaped { spec, .. }
            | DragSpec::Open { spec, .. } => walk(spec, placed, next, into),
            DragSpec::Free { .. }
            | DragSpec::Floating
            | DragSpec::Into { .. }
            | DragSpec::Custom { .. } => {}
        }
    }
    let mut into = Vec::new();
    walk(spec, placed, &mut 0, &mut into);
    into
}

/// How far, in points, a keyboard step moves an item over the first `free` place in `spec`,
/// if it has one: a keyboard carry with no fixed place the way it steps moves that far.
pub fn free_step<D>(spec: &DragSpec<D>) -> Option<f32> {
    match spec {
        DragSpec::Free { step, .. } => Some(*step),
        DragSpec::Closest { of, .. } => of.iter().find_map(free_step),
        DragSpec::WhenFar { near, far, .. } => free_step(near).or_else(|| free_step(far)),
        DragSpec::WhenHeld {
            held, otherwise, ..
        } => free_step(held).or_else(|| free_step(otherwise)),
        DragSpec::SnapRadius { spec, .. }
        | DragSpec::Thrown { spec, .. }
        | DragSpec::Following { spec, .. }
        | DragSpec::Shaped { spec, .. }
        | DragSpec::Open { spec, .. } => free_step(spec),
        DragSpec::Fixed(_)
        | DragSpec::Floating
        | DragSpec::Into { .. }
        | DragSpec::Custom { .. } => None,
    }
}

/// Every node `spec` places relative to, in the spec's order: what a board needs to know the
/// window of, so a place in one window is never chosen by a pointer in another.
pub fn nodes<D>(spec: &DragSpec<D>) -> Vec<NodeRef> {
    fn walk<D>(spec: &DragSpec<D>, into: &mut Vec<NodeRef>) {
        match spec {
            DragSpec::Fixed(candidate) => into.push(candidate.node),
            DragSpec::Closest { of, .. } => of.iter().for_each(|spec| walk(spec, into)),
            DragSpec::WhenFar { near, far, .. } => {
                walk(near, into);
                walk(far, into);
            }
            DragSpec::WhenHeld {
                held, otherwise, ..
            } => {
                walk(held, into);
                walk(otherwise, into);
            }
            DragSpec::SnapRadius { spec, .. }
            | DragSpec::Thrown { spec, .. }
            | DragSpec::Following { spec, .. }
            | DragSpec::Open { spec, .. } => walk(spec, into),
            DragSpec::Shaped { spec, within } => {
                into.push(*within);
                walk(spec, into);
            }
            DragSpec::Free { area, .. } => into.push(*area),
            DragSpec::Into { region, .. } => into.push(*region),
            DragSpec::Custom { node, .. } => into.push(*node),
            DragSpec::Floating => {}
        }
    }
    let mut into = Vec::new();
    walk(spec, &mut into);
    into
}

/// The place a keyboard step `toward` takes from `from`: of the places that lie that way, the
/// one nearest along the arrow, a step across counting twice as far. A place chosen nearest an
/// edge (a pane's split, `Candidate::nearest_edge`) answers only the arrow that edge faces, so
/// Right reaches "right of" a pane and never its band along the top.
pub fn step_toward(rests: &[(Rect, Option<Side>)], from: Pos2, toward: Vec2) -> Option<usize> {
    let toward = toward / toward.length().max(f32::EPSILON);
    let facing = |side: Side| {
        let out = match side {
            Side::Left => Vec2::new(-1.0, 0.0),
            Side::Right => Vec2::new(1.0, 0.0),
            Side::Above => Vec2::new(0.0, -1.0),
            Side::Below => Vec2::new(0.0, 1.0),
        };
        out.dot(toward) > 0.5
    };
    rests
        .iter()
        .enumerate()
        .filter(|(_, (_, edge))| edge.is_none_or(facing))
        .filter_map(|(at, (rest, _))| {
            let away = rest.center() - from;
            let length = away.length();
            let along = away.dot(toward);
            let across = (away - toward * along).length();
            (length > 0.5 && along >= 0.5 * length).then_some((at, along + 2.0 * across))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(at, _)| at)
}

/// `item` moved the least that puts it inside `area`, or centred on an axis it is too long for.
fn clamp_into(item: Rect, area: Rect) -> Rect {
    let axis = |min: f32, length: f32, low: f32, high: f32| {
        if length >= high - low {
            low + (high - low - length) / 2.0
        } else {
            min.clamp(low, high - length)
        }
    };
    let x = axis(item.min.x, item.width(), area.min.x, area.max.x);
    let y = axis(item.min.y, item.height(), area.min.y, area.max.y);
    Rect::from_min_size(Pos2::new(x, y), item.size())
}

/// A place in a list, named by the row it comes after (`None`: first), as the storage layer
/// names it: a peer's insert does not move it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct After<K>(pub Option<K>);

/// Moves `item` to just after `after` in `order` (to the front for `None`), adding it if it
/// was not there. `after` naming `item` itself, or a key not in `order`, leaves it where it is.
pub fn move_after<K: PartialEq>(order: &mut Vec<K>, item: K, after: Option<K>) {
    if after.as_ref() == Some(&item) {
        return;
    }
    if let Some(after) = &after
        && !order.contains(after)
    {
        return;
    }
    order.retain(|shown| *shown != item);
    let at = match &after {
        Some(after) => order
            .iter()
            .position(|shown| shown == after)
            .map_or(0, |at| at + 1),
        None => 0,
    };
    order.insert(at, item);
}
