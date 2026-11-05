//! Hidden bands: an auto-hidden band of bars, and every band while a full screen window covers
//! its display, comes out from its edge of the display when the pointer pushes on past the edge
//! or a finger swipes in from it, and goes away once the pointer has been away from it a moment.
//!
//! The dock hides the band and slides it out as far as asked (`DockHostLayout::hidden_bands`,
//! `reveal`). The host walls the pointer at each band's edge and says how far it was pushed or
//! pulled (`desktop::displays::set_edges`, `ScreenEdgesWatch`). What is left is here: how far a
//! push or a swipe brings the band out, when it opens, and when it goes.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use be_sdk_beui::beui::dock::DockEdge;
use be_sdk_beui::beui::dock_host::styled::{DockHostLayout, HiddenBand};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Rect, vec2};
use be_sdk_beui::desktop;
use be_sdk_beui::{
    DisplaysWatch, Editor, ScreenEdge, ScreenEdgeState, ScreenEdgesWatch, ScreenSide,
};

/// How much of the band a push or a swipe must bring out for it to open the rest of the way.
const OPENS_AT: f32 = 0.5;
/// How long after the last push the pushed distance starts draining.
const DRAINS_AFTER: Duration = Duration::from_millis(120);
/// How fast pushed distance drains, in band thicknesses a second.
const DRAIN: f32 = 2.0;
/// How fast the band slides once it opens or goes, in its thickness a second.
const SLIDE: f32 = 6.0;
/// How long the pointer stays away before an open band goes.
const GOES_AFTER: Duration = Duration::from_millis(500);
/// How far past an open band the pointer still counts as near it, in band thicknesses.
const KEEP: f32 = 1.0;
const TICK: Duration = Duration::from_millis(16);

/// The screen edges of every display's hidden bands, sent to the host as one list.
#[derive(Clone, Copy)]
pub(crate) struct HiddenEdges {
    editor: Editor,
    reported: Signal<BTreeMap<u32, ScreenEdge>>,
    states: Memo<Vec<ScreenEdgeState>>,
    offered: Memo<bool>,
}

impl HiddenEdges {
    pub(crate) fn new(editor: Editor) -> Self {
        let edges = Self {
            editor,
            reported: create_signal(BTreeMap::new()),
            states: editor.watch::<ScreenEdgesWatch>(),
            offered: editor.offered::<ScreenEdgesWatch>(),
        };
        let sent = store(RefCell::new(None::<Vec<ScreenEdge>>));
        create_effect(move || {
            if !edges.offered.get() {
                return;
            }
            let list: Vec<ScreenEdge> = edges
                .reported
                .with(|reported| reported.values().cloned().collect());
            untrack(|| {
                let sent = sent.value();
                if sent.borrow().as_ref() == Some(&list) {
                    return;
                }
                *sent.borrow_mut() = Some(list.clone());
                editor.act(move || desktop::displays::set_edges(list).detach());
            });
        });
        edges
    }

    fn report(&self, id: u32, edge: Option<ScreenEdge>) {
        if !self.reported.is_alive()
            || self
                .reported
                .with_untracked(|reported| reported.get(&id) == edge.as_ref())
        {
            return;
        }
        self.reported.update(|reported| match edge {
            Some(edge) => {
                reported.insert(id, edge);
            }
            None => {
                reported.remove(&id);
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

fn edge_id(display: u32, edge: DockEdge) -> u32 {
    let side = match edge {
        DockEdge::Top => 0,
        DockEdge::Bottom => 1,
        DockEdge::Left => 2,
        DockEdge::Right => 3,
    };
    display * 4 + side
}

fn across(edge: DockEdge, rect: Rect) -> f32 {
    match edge {
        DockEdge::Top | DockEdge::Bottom => rect.height(),
        DockEdge::Left | DockEdge::Right => rect.width(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Motion {
    /// Pushed distance not yet drained, in points.
    pressure: f32,
    last_push: Option<Instant>,
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
        edges
            .states
            .with(|states| states.iter().find(|state| state.id == id).cloned())
    });
    let open = create_signal(false);
    let motion = StoredCell::new(Motion::default());
    let seen = StoredCell::new(None::<(f32, u32, u32)>);
    // Opened by a finger, which has no pointer to go away: it goes when a finger lands elsewhere.
    let by_touch = create_signal(false);
    let preferences = use_preferences();
    let animates = create_memo(move || preferences.get().motion.animates());
    let thickness = move || {
        band.get_untracked()
            .map_or(0.0, |band| across(edge, band.rect))
            .max(1.0)
    };
    let covered = move || band.get_untracked().is_some_and(|band| band.covered);

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
            (false, None) => {
                let lead = match covered() {
                    true => thick,
                    false => 0.0,
                };
                (held.pressure - lead) / thick
            }
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
        busy
    };
    let ticking = create_timer(move || step().then_some(TICK));
    let nudge = move || {
        if !ticking.running() {
            ticking.start(Duration::ZERO);
        }
    };

    create_effect(move || {
        let Some(state) = heard.get() else {
            return;
        };
        untrack(|| {
            let before = seen.replace(Some((state.pushed, state.pulls, state.taps_away)));
            let mut held = motion.get();
            if let Some((pushed, pulls, taps)) = before {
                if state.taps_away > taps && by_touch.get() {
                    by_touch.set(false);
                    open.set(false);
                }
                let push = match state.pushed >= pushed {
                    true => state.pushed - pushed,
                    false => state.pushed,
                };
                if push > 0.0 && !open.get() {
                    held.pressure += push;
                    held.last_push = Some(now());
                }
                if state.pulls > pulls {
                    held.pulling = None;
                    if state.pulled >= thickness() * OPENS_AT {
                        by_touch.set(true);
                        open.set(true);
                    } else if !open.get() {
                        held.pressure = 0.0;
                    }
                }
            }
            if let Some(pulled) = state.pull
                && !open.get()
            {
                held.pulling = Some(pulled);
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

    let near = create_memo(move || heard.get().is_some_and(|state| state.near));
    let going = create_timer(move || {
        if open.get_untracked() && !held_out.get_untracked() && !near.get_untracked() {
            open.set(false);
        }
        None
    });
    create_effect(move || {
        if !open.get() {
            untrack(|| by_touch.set(false));
        }
    });
    create_effect(move || {
        let stays = !open.get() || held_out.get() || near.get() || by_touch.get();
        untrack(|| match stays {
            true => going.stop(),
            false => going.restart(GOES_AFTER),
        });
    });

    create_effect(move || {
        let reported = band.get().zip(screen.get()).map(|(band, screen)| {
            let rect = band.rect.translate(screen.min.to_vec2());
            let (side, start, end) = match edge {
                DockEdge::Top => (ScreenSide::Top, rect.left(), rect.right()),
                DockEdge::Bottom => (ScreenSide::Bottom, rect.left(), rect.right()),
                DockEdge::Left => (ScreenSide::Left, rect.top(), rect.bottom()),
                DockEdge::Right => (ScreenSide::Right, rect.top(), rect.bottom()),
            };
            let reach = across(edge, rect) * KEEP;
            let open = open.get();
            ScreenEdge {
                id,
                display: screen,
                side,
                start,
                end,
                wall: !open,
                deliberate: band.covered,
                keep: open.then(|| {
                    Rect::from_min_max(rect.min - vec2(reach, reach), rect.max + vec2(reach, reach))
                }),
            }
        });
        untrack(|| edges.report(id, reported));
    });
    on_cleanup(move || {
        edges.report(id, None);
        layout.reveal(edge, 0.0);
    });
    view! {
        <Frame />
    }
}
