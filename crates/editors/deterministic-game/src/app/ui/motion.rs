use block_editor_beui::beui::CanvasView;
use std::time::{Duration, Instant};

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{Pos2, Rect};
use game_api::board::{HandCard, ItemId, Sprite};

use super::hand::hand_rects;
use super::layout::Layout;

const FLIGHT: Duration = Duration::from_millis(260);
const STILL: f32 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sighting {
    pub(crate) id: ItemId,
    pub(crate) sprite: Sprite,
    pub(crate) rect: Rect,
}

pub(crate) fn sightings(
    layout: &Layout,
    view: CanvasView,
    hand: &[HandCard],
    bar: Rect,
) -> Vec<Sighting> {
    let table = layout.items.iter().map(|placed| Sighting {
        id: placed.id,
        sprite: placed.sprite.clone(),
        rect: view.rect_to_screen(placed.rect),
    });
    let held = hand
        .iter()
        .zip(hand_rects(bar, hand.len()))
        .map(|(held, rect)| Sighting {
            id: held.id,
            sprite: held.sprite.clone(),
            rect,
        });
    table.chain(held).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Flight {
    pub(crate) id: ItemId,
    pub(crate) sprite: Sprite,
    pub(crate) from: Rect,
    pub(crate) to: Rect,
    pub(crate) start: Instant,
}

impl Flight {
    fn progress(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.start).as_secs_f32();
        tween(elapsed, FLIGHT.as_secs_f32(), Easing::Linear)
    }

    pub(crate) fn rect(&self, now: Instant) -> Rect {
        let along = Easing::OutCubic.sample(self.progress(now));
        let between = |from: Pos2, to: Pos2| from + (to - from) * along;
        Rect::from_min_max(
            between(self.from.min, self.to.min),
            between(self.from.max, self.to.max),
        )
    }

    fn distance(&self) -> f32 {
        (self.to.center() - self.from.center()).length()
    }

    fn landed(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }
}

pub(crate) fn flights(
    before: &[Sighting],
    after: &[Sighting],
    dropped: Option<(ItemId, Rect)>,
    start: Instant,
) -> Vec<Flight> {
    after
        .iter()
        .filter_map(|seen| {
            let from = match dropped {
                Some((id, rect)) if id == seen.id => rect,
                _ => {
                    let was = before.iter().find(|was| was.id == seen.id)?;
                    let moved = (was.rect.min - seen.rect.min).length()
                        + (was.rect.max - seen.rect.max).length();
                    if moved < STILL {
                        return None;
                    }
                    was.rect
                }
            };
            Some(Flight {
                id: seen.id,
                sprite: seen.sprite.clone(),
                from,
                to: seen.rect,
                start,
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(crate) struct Motion {
    pub(crate) flights: Signal<Vec<Flight>>,
    pub(crate) clock: Signal<Instant>,
    pub(crate) flying: Memo<Vec<ItemId>>,
    timer: Animation,
}

impl Motion {
    pub(crate) fn new() -> Self {
        let flights = create_signal(Vec::<Flight>::new());
        let clock = create_signal(now());
        let timer = create_animation(
            move |frame| {
                let now = frame.now;
                clock.set(now);
                let mut flying = false;
                flights.update(|flights| {
                    flights.retain(|flight| !flight.landed(now));
                    flying = !flights.is_empty();
                });
                flying
            },
            move || {
                clock.set(now());
                flights.set(Vec::new());
            },
        );
        let flying = create_memo(move || {
            flights.with(|flights| flights.iter().map(|flight| flight.id).collect::<Vec<_>>())
        });
        Self {
            flights,
            clock,
            flying,
            timer,
        }
    }

    pub(crate) fn launch(&self, launched: Vec<Flight>) {
        if launched.is_empty() {
            return;
        }
        let now = launched[0].start;
        self.clock.set(now);
        self.flights.update(|flights| {
            flights.retain(|flight| launched.iter().all(|new| new.id != flight.id));
            flights.extend(launched);
            flights.sort_by(|one, other| one.distance().total_cmp(&other.distance()));
        });
        self.timer.start();
    }
}
