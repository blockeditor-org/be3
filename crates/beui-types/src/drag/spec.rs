//! Drags as the places they can lead to: a spec names where a carried item can end up, and
//! evaluating it for a point gives where the item draws and which place a release takes. Pure:
//! nodes are placed through the function the board hands in, so it is tested without a core.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use be_lib_geometry::{Pos2, Rect, Vec2};

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
    /// Anywhere in `area`: the pointer minus the grab, clamped to it; `drop` names the rect.
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
    /// A nested site: chosen while the pointer is in `region`, which chooses from there on.
    Into { region: NodeRef, drop: D },
    /// Anything else, from the point and the last frame.
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
    let found = walk(spec, evaluation, last, carried, &mut next);
    match found {
        Some(found) => Evaluated {
            frame: DragFrame {
                item: found.item,
                chosen: found.chosen,
                floating: found.floating,
                chain: found.chain,
            },
            drop: found.drop,
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

struct Found<D> {
    item: Rect,
    chosen: Option<CandidateId>,
    drop: Option<D>,
    floating: bool,
    chain: bool,
    /// How far the place is from the carried item: what `closest` and `when_far` compare.
    distance: f32,
}

fn floated<D>(carried: Rect) -> Found<D> {
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
fn walk<D: Clone>(
    spec: &DragSpec<D>,
    evaluation: &Evaluation,
    last: &DragFrame,
    carried: Rect,
    next: &mut usize,
) -> Option<Found<D>> {
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
                None => distance(carried, rest),
            };
            Some(Found {
                item: rest,
                chosen: Some(id),
                drop: Some(candidate.drop.clone()),
                floating: false,
                chain: false,
                distance,
            })
        }
        DragSpec::Closest { of, sticky } => {
            let found: Vec<Found<D>> = of
                .iter()
                .filter_map(|spec| walk(spec, evaluation, last, carried, next))
                .collect();
            let held = found
                .iter()
                .position(|found| found.chosen.is_some() && found.chosen == last.chosen);
            let nearest = found
                .iter()
                .enumerate()
                .filter(|(_, found)| found.distance.is_finite())
                .min_by(|(_, left), (_, right)| left.distance.total_cmp(&right.distance))
                .map(|(at, _)| at);
            let pick = match (held, nearest) {
                (Some(held), Some(nearest))
                    if found[held].distance.is_finite()
                        && found[nearest].distance + sticky >= found[held].distance =>
                {
                    held
                }
                (_, Some(nearest)) => nearest,
                (Some(held), None) => held,
                (None, None) => return None,
            };
            found.into_iter().nth(pick)
        }
        DragSpec::Free { area, drop } => {
            let (area, _) = (evaluation.placed)(area.try_get()?)?;
            let item = clamp_into(carried, area);
            Some(Found {
                item,
                chosen: Some(id),
                drop: Some(drop(item)),
                floating: false,
                chain: false,
                distance: distance(carried, item),
            })
        }
        DragSpec::WhenFar { near, far, reach } => {
            let near = walk(near, evaluation, last, carried, next);
            // Ids stay the same whichever side is chosen, so count `far`'s even when unused.
            let far = walk(far, evaluation, last, carried, next);
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
            let found = walk(spec, evaluation, last, carried, next)?;
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
                return walk(spec, evaluation, last, carried, next);
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
            walk(spec, &projected, last, carried.translate(ahead), next)
        }
        DragSpec::Into { region, drop } => {
            let node = region.try_get()?;
            let (rect, _) = (evaluation.placed)(node)?;
            let inside = rect.contains(evaluation.point.pos);
            Some(Found {
                item: carried,
                chosen: Some(id),
                drop: Some(drop.clone()),
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
        Some(after) => order.iter().position(|shown| shown == after).map_or(0, |at| at + 1),
        None => 0,
    };
    order.insert(at, item);
}
