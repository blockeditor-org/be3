//! Hidden bands: an auto-hidden band of bars, and every band while a full screen window covers
//! its display, comes out from its edge of the display when the pointer, resting against the edge,
//! pushes on past it, or when a finger swipes in from it. It slides away as soon as nothing keeps
//! it out: the pointer has left it and no bar in it holds it out (a menu or panel open from it).
//! The pointer coming back before it is gone brings it out again.
//!
//! The dock hides the band and slides it out as far as asked (`DockHostLayout::hidden_bands`,
//! `reveal`). The host walls the pointer where asked and says how far it was pushed on past each
//! wall, whether the pointer is in each region asked about, and hands over the fingers landing in
//! regions that capture them (`desktop::displays::set_screen_input`, `ScreenInputWatch`). Every
//! rule about what that means is here.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use be_sdk_beui::beui::dock::DockEdge;
use be_sdk_beui::beui::dock_host::styled::{DockHostLayout, HiddenBand};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Pos2, Rect, pos2, vec2};
use be_sdk_beui::desktop;
use be_sdk_beui::{
    DisplaysWatch, Editor, ScreenInput, ScreenInputState, ScreenInputWatch, ScreenRegion,
    ScreenRegionState, ScreenSide, ScreenWall, ScreenWallState,
};

/// How long the pointer rests against an edge before its pushes count, so a sweep that ends at the
/// edge never counts, however fast it was.
const SETTLE: Duration = Duration::from_millis(150);
/// How much of the band a push or a swipe must bring out for it to open the rest of the way.
const OPENS_AT: f32 = 0.5;
/// How long after the last push the pushed distance starts draining.
const DRAINS_AFTER: Duration = Duration::from_millis(120);
/// How fast pushed distance drains, in band thicknesses a second.
const DRAIN: f32 = 2.0;
/// How fast the band slides once it opens or goes, in its thickness a second.
const SLIDE: f32 = 6.0;
/// How far past an open band the pointer still counts as near it, in band thicknesses.
const KEEP: f32 = 1.0;
/// How far in from an edge a finger may land to swipe it, in points: little, since what lies under
/// a hidden band may have its own controls near the edge.
const SWIPE_REACH: f32 = 6.0;
const TICK: Duration = Duration::from_millis(16);

/// What one band asks of the host: a wall while it is hidden, and regions.
#[derive(Clone, Debug, Default, PartialEq)]
struct Asked {
    wall: Option<ScreenWall>,
    regions: Vec<ScreenRegion>,
}

/// The walls and regions of every display's hidden bands, sent to the host as one.
#[derive(Clone, Copy)]
pub(crate) struct HiddenEdges {
    editor: Editor,
    asked: Signal<BTreeMap<u32, Asked>>,
    state: Memo<ScreenInputState>,
    offered: Memo<bool>,
}

impl HiddenEdges {
    pub(crate) fn new(editor: Editor) -> Self {
        let edges = Self {
            editor,
            asked: create_signal(BTreeMap::new()),
            state: editor.watch::<ScreenInputWatch>(),
            offered: editor.offered::<ScreenInputWatch>(),
        };
        let sent = store(RefCell::new(None::<ScreenInput>));
        create_effect(move || {
            if !edges.offered.get() {
                return;
            }
            let input = edges.asked.with(|asked| ScreenInput {
                walls: asked
                    .values()
                    .filter_map(|asked| asked.wall.clone())
                    .collect(),
                regions: asked
                    .values()
                    .flat_map(|asked| asked.regions.iter().cloned())
                    .collect(),
            });
            untrack(|| {
                let sent = sent.value();
                if sent.borrow().as_ref() == Some(&input) {
                    return;
                }
                *sent.borrow_mut() = Some(input.clone());
                editor.act(move || desktop::displays::set_screen_input(input).detach());
            });
        });
        edges
    }

    fn ask(&self, id: u32, asked: Option<Asked>) {
        if !self.asked.is_alive()
            || self
                .asked
                .with_untracked(|held| held.get(&id) == asked.as_ref())
        {
            return;
        }
        self.asked.update(|held| match asked {
            Some(asked) => {
                held.insert(id, asked);
            }
            None => {
                held.remove(&id);
            }
        });
    }
}

/// Lets `layout`'s bands hide, and brings out each hidden one on `display`.
#[component]
pub(crate) fn HiddenBands(edges: HiddenEdges, layout: DockHostLayout, display: u32) -> NodeId {
    layout.offer_hidden_bands();
    let hidden = create_memo(move || {
        layout
            .hidden_bands()
            .into_iter()
            .map(|band| band.edge)
            .collect::<Vec<_>>()
    });
    view! {
        <Layers>
            <ForEach keys={hidden}>|edge: DockEdge|
                <HiddenBandEdge edges layout display edge />
            </ForEach>
        </Layers>
    }
}

/// Where a band is, and whether it is held out or covers a full screen window.
fn band_of(layout: DockHostLayout, edge: DockEdge) -> Option<HiddenBand> {
    layout
        .hidden_bands()
        .into_iter()
        .find(|band| band.edge == edge)
}

/// The id of the wall along `edge` of `display`; its regions are the two after twice it.
fn edge_id(display: u32, edge: DockEdge) -> u32 {
    let side = match edge {
        DockEdge::Top => 0,
        DockEdge::Bottom => 1,
        DockEdge::Left => 2,
        DockEdge::Right => 3,
    };
    display * 4 + side
}

/// The strip along the edge a finger swipes the band out from.
fn swipe_id(id: u32) -> u32 {
    id * 2
}

/// The band and a thickness around it, where the pointer keeps it out.
fn keep_id(id: u32) -> u32 {
    id * 2 + 1
}

fn across(edge: DockEdge, rect: Rect) -> f32 {
    match edge {
        DockEdge::Top | DockEdge::Bottom => rect.height(),
        DockEdge::Left | DockEdge::Right => rect.width(),
    }
}

/// How far in from `edge` of `screen` a point is.
fn inward(edge: DockEdge, screen: Rect, at: Pos2) -> f32 {
    match edge {
        DockEdge::Top => at.y - screen.top(),
        DockEdge::Bottom => screen.bottom() - at.y,
        DockEdge::Left => at.x - screen.left(),
        DockEdge::Right => screen.right() - at.x,
    }
}

/// What the host said about one band's wall and regions.
#[derive(Clone, Debug, Default, PartialEq)]
struct Heard {
    wall: Option<ScreenWallState>,
    swipe: Option<ScreenRegionState>,
    keep: Option<ScreenRegionState>,
    touch_downs: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Motion {
    /// Pushed distance not yet drained, in points.
    pressure: f32,
    last_push: Option<Instant>,
    /// Since when the pointer has rested against the edge.
    resting: Option<Instant>,
    /// How far in from the edge a swiping finger is.
    pulling: Option<f32>,
    /// How far out the band is.
    amount: f32,
    ticked: Option<Instant>,
}

#[component]
fn HiddenBandEdge(
    edges: HiddenEdges,
    layout: DockHostLayout,
    display: u32,
    edge: DockEdge,
) -> NodeId {
    let id = edge_id(display, edge);
    let band = create_memo(move || band_of(layout, edge));
    let displays = edges.editor.watch::<DisplaysWatch>();
    let screen = create_memo(move || {
        displays.with(|displays| displays.get(display as usize).map(|shown| shown.rect))
    });
    let heard = create_memo(move || {
        edges.state.with(|state| {
            let region = |id: u32| state.regions.iter().find(|region| region.id == id).cloned();
            Heard {
                wall: state.walls.iter().find(|wall| wall.id == id).cloned(),
                swipe: region(swipe_id(id)),
                keep: region(keep_id(id)),
                touch_downs: state.touch_downs,
            }
        })
    });
    let open = create_signal(false);
    // Sliding away after it was open: the pointer coming back near brings it out again.
    let closing = create_signal(false);
    let motion = StoredCell::new(Motion::default());
    let seen = StoredCell::new(None::<Heard>);
    // Opened by a finger, which has no pointer to go away: it goes when a finger lands elsewhere.
    let by_touch = create_signal(false);
    let preferences = use_preferences();
    let animates = create_memo(move || preferences.get().motion.animates());
    let thickness = move || {
        band.get_untracked()
            .map_or(0.0, |band| across(edge, band.rect))
            .max(1.0)
    };

    let step = move || -> bool {
        let now = now();
        let mut held = motion.get();
        let delta = held.ticked.map_or(0.0, |ticked| {
            now.saturating_duration_since(ticked).as_secs_f32()
        });
        held.ticked = Some(now);
        let thick = thickness();
        let is_open = open.get_untracked();
        let drains = held
            .last_push
            .is_none_or(|last| now.saturating_duration_since(last) >= DRAINS_AFTER);
        if drains && held.pressure > 0.0 {
            held.pressure = (held.pressure - DRAIN * thick * delta).max(0.0);
        }
        let following = !is_open && (held.pressure > 0.0 || held.pulling.is_some());
        let goal = match (is_open, held.pulling) {
            (true, _) => 1.0,
            (false, Some(pulled)) => pulled / thick,
            // The first band's worth of a push brings nothing out, so only a push meant for it,
            // not a pointer overshooting on its way to something by the edge, shows the band.
            (false, None) => (held.pressure - thick) / thick,
        }
        .clamp(0.0, 1.0);
        if following && held.pulling.is_none() && goal >= OPENS_AT {
            held.pressure = 0.0;
            motion.set(held);
            open.set(true);
            return true;
        }
        held.amount = match following || !animates.get_untracked() {
            true => goal,
            false => match held.amount < goal {
                true => (held.amount + SLIDE * delta).min(goal),
                false => (held.amount - SLIDE * delta).max(goal),
            },
        };
        let busy = held.pressure > 0.0 || held.amount != goal;
        if !busy {
            held.ticked = None;
        }
        motion.set(held);
        layout.reveal(edge, held.amount);
        if closing.get_untracked() && (is_open || held.amount == 0.0) {
            closing.set(false);
        }
        busy
    };
    let ticking = create_timer(move || step().then_some(TICK));
    let nudge = move || {
        if !ticking.running() {
            ticking.start(Duration::ZERO);
        }
    };

    create_effect(move || {
        let heard = heard.get();
        untrack(|| {
            let Some(screen) = screen.get() else {
                return;
            };
            let before = seen.replace(Some(heard.clone())).unwrap_or_default();
            let now = now();
            let mut held = motion.get();
            let settled = held
                .resting
                .is_some_and(|since| now.saturating_duration_since(since) >= SETTLE);
            if let Some(wall) = &heard.wall {
                let push = match &before.wall {
                    Some(was) if wall.pushed >= was.pushed => wall.pushed - was.pushed,
                    _ => wall.pushed,
                };
                if push > 0.0 && settled && !open.get() {
                    held.pressure += push;
                    held.last_push = Some(now);
                }
            }
            held.resting = match heard.wall.as_ref().is_some_and(|wall| wall.against) {
                true => held.resting.or(Some(now)),
                false => None,
            };
            let landed = heard.touch_downs.saturating_sub(before.touch_downs);
            let landed_near = match (&heard.keep, &before.keep) {
                (Some(keep), Some(was)) => keep.touch_downs.saturating_sub(was.touch_downs),
                _ => 0,
            };
            if landed > landed_near && by_touch.get() {
                by_touch.set(false);
                open.set(false);
            }
            if let Some(swipe) = &heard.swipe {
                let lifts = before.swipe.as_ref().map_or(0, |was| was.lifts);
                if swipe.lifts > lifts {
                    held.pulling = None;
                    let pulled = swipe
                        .lifted
                        .map_or(0.0, |lifted| inward(edge, screen, lifted));
                    if pulled >= thickness() * OPENS_AT {
                        by_touch.set(true);
                        open.set(true);
                    } else if !open.get() {
                        held.pressure = 0.0;
                    }
                }
                if let Some(touch) = swipe.touch
                    && !open.get()
                {
                    held.pulling = Some(inward(edge, screen, touch).max(0.0));
                }
            }
            motion.set(held);
            nudge();
        });
    });

    let held_out = create_memo(move || band.get().is_some_and(|band| band.held));
    create_effect(move || {
        if held_out.get() {
            untrack(|| open.set(true));
        }
    });
    create_effect(move || {
        open.get();
        untrack(nudge);
    });

    let near = create_memo(move || {
        heard.with(|heard| heard.keep.as_ref().is_some_and(|keep| keep.pointer))
    });
    create_effect(move || {
        if !open.get() {
            untrack(|| by_touch.set(false));
        }
    });
    // Goes the moment nothing keeps it: not when nothing has kept it yet, as just after a push
    // opens it and before the host says the pointer is near.
    let stays = create_memo(move || held_out.get() || near.get() || by_touch.get());
    let kept = StoredCell::new(false);
    create_effect(move || {
        let stays = stays.get();
        untrack(|| {
            let was = kept.replace(stays);
            if was && !stays && open.get() {
                closing.set(animates.get());
                open.set(false);
            }
        });
    });
    create_effect(move || {
        if closing.get() && near.get() {
            untrack(|| open.set(true));
        }
    });

    create_effect(move || {
        let asked = band.get().zip(screen.get()).map(|(band, screen)| {
            let rect = band.rect.translate(screen.min.to_vec2());
            let (side, start, end) = match edge {
                DockEdge::Top => (ScreenSide::Top, rect.left(), rect.right()),
                DockEdge::Bottom => (ScreenSide::Bottom, rect.left(), rect.right()),
                DockEdge::Left => (ScreenSide::Left, rect.top(), rect.bottom()),
                DockEdge::Right => (ScreenSide::Right, rect.top(), rect.bottom()),
            };
            let strip = match edge {
                DockEdge::Top => Rect::from_min_max(
                    pos2(start, screen.top()),
                    pos2(end, screen.top() + SWIPE_REACH),
                ),
                DockEdge::Bottom => Rect::from_min_max(
                    pos2(start, screen.bottom() - SWIPE_REACH),
                    pos2(end, screen.bottom()),
                ),
                DockEdge::Left => Rect::from_min_max(
                    pos2(screen.left(), start),
                    pos2(screen.left() + SWIPE_REACH, end),
                ),
                DockEdge::Right => Rect::from_min_max(
                    pos2(screen.right() - SWIPE_REACH, start),
                    pos2(screen.right(), end),
                ),
            };
            let reach = across(edge, rect) * KEEP;
            let open = open.get();
            let mut regions = Vec::new();
            if !open {
                regions.push(ScreenRegion {
                    id: swipe_id(id),
                    rect: strip,
                    captures_touch: true,
                });
            }
            if open || closing.get() {
                regions.push(ScreenRegion {
                    id: keep_id(id),
                    rect: Rect::from_min_max(
                        rect.min - vec2(reach, reach),
                        rect.max + vec2(reach, reach),
                    ),
                    captures_touch: false,
                });
            }
            Asked {
                wall: (!open).then_some(ScreenWall {
                    id,
                    display: screen,
                    side,
                    start,
                    end,
                }),
                regions,
            }
        });
        untrack(|| edges.ask(id, asked));
    });
    on_cleanup(move || {
        edges.ask(id, None);
        layout.reveal(edge, 0.0);
    });
    view! {
        <Frame />
    }
}
