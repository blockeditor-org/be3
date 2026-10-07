use std::time::{Duration, Instant};

use block_editor_beui::beui::reactive::{
    CanvasView, Memo, ReadSignal, Timer, WriteSignal, clone, create_memo, create_signal,
    create_timer, now, with_document,
};
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
        (elapsed / FLIGHT.as_secs_f32()).clamp(0.0, 1.0)
    }

    pub(crate) fn rect(&self, now: Instant) -> Rect {
        let along = 1.0 - (1.0 - self.progress(now)).powi(3);
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

#[derive(Clone)]
pub(crate) struct Motion {
    pub(crate) flights: ReadSignal<Vec<Flight>>,
    set_flights: WriteSignal<Vec<Flight>>,
    pub(crate) clock: ReadSignal<Instant>,
    set_clock: WriteSignal<Instant>,
    pub(crate) flying: Memo<Vec<ItemId>>,
    timer: Timer,
}

impl Motion {
    pub(crate) fn new() -> Self {
        let (flights, set_flights) = create_signal(Vec::<Flight>::new());
        let (clock, set_clock) = create_signal(now());
        let timer = create_timer(clone!(set_flights set_clock -> move || {
            let now = now();
            set_clock.set(now);
            let mut flying = false;
            set_flights.update(|flights| {
                flights.retain(|flight| !flight.landed(now));
                flying = !flights.is_empty();
            });
            flying.then_some(Duration::ZERO)
        }));
        let flying = create_memo(clone!(flights -> move || {
            flights.with(|flights| flights.iter().map(|flight| flight.id).collect::<Vec<_>>())
        }));
        Self {
            flights,
            set_flights,
            clock,
            set_clock,
            flying,
            timer,
        }
    }

    pub(crate) fn launch(&self, launched: Vec<Flight>) {
        if launched.is_empty() || !with_document(|document| document.motion().animates()) {
            return;
        }
        let now = launched[0].start;
        self.set_clock.set(now);
        self.set_flights.update(|flights| {
            flights.retain(|flight| launched.iter().all(|new| new.id != flight.id));
            flights.extend(launched);
            flights.sort_by(|one, other| one.distance().total_cmp(&other.distance()));
        });
        self.timer.start(Duration::ZERO);
    }
}
