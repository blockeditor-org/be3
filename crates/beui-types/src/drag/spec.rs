//! Drags as the places they can lead to: a spec names where a carried item can end up, and
//! evaluating it for a point gives where the item draws and which place a release takes. Pure:
//! nodes are placed through the function the board hands in, so it is tested without a core.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use be_lib_geometry::{Direction, Pos2, Rect, Vec2};

use super::DragPoint;
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
/// How long, in seconds, a thrown drag is carried on along its path to choose where it lands.
pub const THROW_SECONDS: f32 = 0.15;

/// A place a carried item can land: what dropping there means (`drop`), where the item would
/// rest, relative to `node`, and where the pointer chooses it, if not by the nearest rest.
#[derive(Clone)]
pub struct Candidate<D> {
    pub drop: D,
    pub rest: Rect,
    pub zone: Option<Rect>,
    pub node: NodeRef,
    pub label: Option<String>,
}

impl<D> Candidate<D> {
    pub fn new(drop: D, rest: Rect, node: NodeRef) -> Self {
        Self {
            drop,
            rest,
            zone: None,
            node,
            label: None,
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
    /// Anywhere in `area`: the pointer minus the grab, clamped to it; `drop` names the rect, in
    /// `area`'s own coordinates as a `Candidate`'s are.
    Free {
        area: NodeRef,
        drop: Rc<dyn Fn(Rect) -> D>,
    },
    /// `near` while the item is within `reach` of where `near` puts it, else `far`.
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
    /// A release faster than `speed` carries on along its path before choosing.
    Thrown { spec: Box<DragSpec<D>>, speed: f32 },
    /// `spec`'s place, with the item drawn under the pointer along `along` and at the place
    /// across it, as a list's row follows the finger while the rows part around it.
    Following {
        spec: Box<DragSpec<D>>,
        along: Direction,
    },
    /// A nested site: chosen while the pointer is in `region`, which chooses from there on.
    Into { region: NodeRef, drop: D },
    /// Anything else, from the point and the last frame. For now it only draws: it chooses
    /// no drop, so a release on it commits nothing.
    Custom(Rc<dyn Fn(DragPoint, &DragFrame) -> DragFrame>),
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
    }
}

pub fn floating<D>() -> DragSpec<D> {
    DragSpec::Floating
}

pub fn into<D>(region: NodeRef, drop: D) -> DragSpec<D> {
    DragSpec::Into { region, drop }
}

pub fn custom<D>(frame: impl Fn(DragPoint, &DragFrame) -> DragFrame + 'static) -> DragSpec<D> {
    DragSpec::Custom(Rc::new(frame))
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
        }
    }

    /// The place this chooses, with the item drawn under the pointer along `along`.
    pub fn following(self, along: Direction) -> Self {
        DragSpec::Following {
            spec: Box::new(self),
            along,
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
            DragSpec::Thrown { spec, .. } => DragSpec::Thrown {
                spec,
                speed: distance,
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
            DragSpec::Free { area, drop: named } => DragSpec::Free {
                area,
                drop: Rc::new(move |rect| drop(named(rect))),
            },
            DragSpec::WhenFar { near, far, reach } => DragSpec::WhenFar {
                near: Box::new(near.map(Rc::clone(&drop))),
                far: Box::new(far.map(drop)),
                reach,
            },
            DragSpec::Floating => DragSpec::Floating,
            DragSpec::SnapRadius {
                spec,
                radius,
                chain,
            } => DragSpec::SnapRadius {
                spec: Box::new(spec.map(drop)),
                radius,
                chain,
            },
            DragSpec::Thrown { spec, speed } => DragSpec::Thrown {
                spec: Box::new(spec.map(drop)),
                speed,
            },
            DragSpec::Following { spec, along } => DragSpec::Following {
                spec: Box::new(spec.map(drop)),
                along,
            },
            DragSpec::Into { region, drop: into } => DragSpec::Into {
                region,
                drop: drop(into),
            },
            DragSpec::Custom(frame) => DragSpec::Custom(frame),
        }
    }
}

/// What a drag lifts: its payload, erased (a board hands it back typed), and the item's rect
/// relative to the pointer, which the item keeps while it floats. With no `item`, an
/// `Interactive` lifting it gives its own node, held where it was pressed. With no
/// `preview`, the source itself is drawn where the item floats.
#[derive(Clone)]
pub struct Carry {
    pub payload: Rc<dyn std::any::Any>,
    pub item: Option<Rect>,
    pub preview: Option<Rc<dyn Fn() -> NodeId>>,
    pub outside: Option<Rc<dyn Fn(DragPoint)>>,
}

impl Carry {
    pub fn new<P: 'static>(payload: P) -> Self {
        Self {
            payload: Rc::new(payload),
            item: None,
            preview: None,
            outside: None,
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
            preview: Some(Rc::new(preview)),
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
                    match zone.contains(evaluation.point.pos) {
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
            })
        }
        DragSpec::Closest { of, sticky } => {
            // The place held from the last frame, the nearest place, and the first floating
            // found: a place always beats floating, so a far site floating never takes the
            // item from a near site's place; it floats only when no place is in reach. A
            // place infinitely far (a zone the pointer left) is let go.
            let mut held: Option<Found<D>> = None;
            let mut nearest: Option<Found<D>> = None;
            let mut floats: Option<Found<D>> = None;
            for spec in of {
                let Some(found) = walk(spec, evaluation, last, carried, next, along) else {
                    continue;
                };
                if !found.placed() {
                    floats.get_or_insert(found);
                    continue;
                }
                if !found.distance.is_finite() {
                    continue;
                }
                if found.chosen == last.chosen {
                    held = Some(found);
                } else if nearest
                    .as_ref()
                    .is_none_or(|nearest| found.distance < nearest.distance)
                {
                    nearest = Some(found);
                }
            }
            match (held, nearest) {
                (Some(held), Some(nearest)) => match nearest.distance + sticky >= held.distance {
                    true => Some(held),
                    false => Some(nearest),
                },
                (Some(held), None) => Some(held),
                (None, Some(nearest)) => Some(nearest),
                (None, None) => floats,
            }
        }
        DragSpec::Free { area, drop } => {
            let (area, scale) = (evaluation.placed)(area.try_get()?)?;
            let item = clamp_into(carried, area);
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
            })
        }
        DragSpec::WhenFar { near, far, reach } => {
            let near = walk(near, evaluation, last, carried, next, along);
            // Ids stay the same whichever side is chosen, so count `far`'s even when unused.
            let far = walk(far, evaluation, last, carried, next, along);
            match near {
                Some(near) if near.distance <= *reach => Some(near),
                _ => far,
            }
        }
        DragSpec::Floating => Some(floated(carried)),
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
        DragSpec::Thrown { spec, speed } => {
            let thrown = evaluation.released && evaluation.velocity.length() >= *speed;
            if !thrown {
                return walk(spec, evaluation, last, carried, next, along);
            }
            let ahead = evaluation.velocity * THROW_SECONDS;
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
            })
        }
        DragSpec::Custom(frame) => {
            let frame = frame(evaluation.point, last);
            Some(Found {
                item: frame.item,
                chosen: frame.chosen,
                drop: None,
                floating: frame.floating,
                chain: frame.chain,
                distance: distance(carried, frame.item),
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
            DragSpec::SnapRadius { spec, .. }
            | DragSpec::Thrown { spec, .. }
            | DragSpec::Following { spec, .. } => walk(spec, placed, next, into),
            DragSpec::Free { .. }
            | DragSpec::Floating
            | DragSpec::Into { .. }
            | DragSpec::Custom(_) => {}
        }
    }
    let mut into = Vec::new();
    walk(spec, placed, &mut 0, &mut into);
    into
}

/// Of `rests`, the nearest whose centre lies towards `toward` from `from` (within 60 degrees
/// of it), as a keyboard step chooses: its index.
pub fn step_toward(rests: &[Rect], from: Pos2, toward: Vec2) -> Option<usize> {
    let toward = toward / toward.length().max(f32::EPSILON);
    rests
        .iter()
        .enumerate()
        .filter_map(|(at, rest)| {
            let away = rest.center() - from;
            let length = away.length();
            (length > 0.5 && away.dot(toward) >= 0.5 * length).then_some((at, length))
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
