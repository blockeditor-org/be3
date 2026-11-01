use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use beui::reactive::*;
use beui::{
    Forwarded, ForwardedInput, Modifiers, NodeId, PointerButton, PointerEvent, PointerId,
    PointerPhase, Rect, Vec2,
};
use uuid::Uuid;

const LATE: Duration = Duration::from_millis(500);
const SIZE_SLACK: f32 = 1.0;

#[derive(Clone, Copy)]
pub(super) struct Receiver {
    pub(super) block: Uuid,
    pub(super) parent: Option<NodeId>,
    pub(super) rect: Memo<Rect>,
    pub(super) used: Memo<Option<Vec2>>,
    pub(super) forward: Callback<ForwardedInput>,
}

struct Queued {
    events: Vec<Forwarded>,
    modifiers: Modifiers,
    rect: Rect,
    since: Instant,
}

thread_local! {
    static RECEIVERS: RefCell<HashMap<NodeId, Receiver>> = RefCell::new(HashMap::new());
    static AWAITED: RefCell<HashMap<Uuid, Callback<NodeId>>> = RefCell::new(HashMap::new());
    static QUEUED: RefCell<HashMap<NodeId, Queued>> = RefCell::new(HashMap::new());
}

pub(super) fn register(node: NodeId, receiver: Receiver) {
    RECEIVERS.with(|receivers| receivers.borrow_mut().insert(node, receiver));
    let awaited = AWAITED.with(|awaited| awaited.borrow_mut().remove(&receiver.block));
    if let Some(hand) = awaited {
        hand.call(node);
    }
}

pub(super) fn forget(node: NodeId) {
    RECEIVERS.with(|receivers| receivers.borrow_mut().remove(&node));
    QUEUED.with(|queued| queued.borrow_mut().remove(&node));
}

pub(super) fn await_receiver(block: Uuid, hand: Callback<NodeId>) {
    AWAITED.with(|awaited| awaited.borrow_mut().insert(block, hand));
}

pub(super) fn stop_awaiting(block: Uuid) {
    AWAITED.with(|awaited| awaited.borrow_mut().remove(&block));
}

pub(super) fn receiver_of(parent: NodeId, block: Uuid) -> Option<NodeId> {
    RECEIVERS.with(|receivers| {
        let receivers = receivers.borrow();
        let all: Vec<(NodeId, Receiver)> = receivers
            .iter()
            .filter(|(node, receiver)| **node != parent && receiver.block == block)
            .map(|(node, receiver)| (*node, *receiver))
            .collect();
        all.iter()
            .find(|(_, receiver)| receiver.parent == Some(parent))
            .or_else(|| all.first())
            .map(|(node, _)| *node)
    })
}

pub(super) fn laid_out(node: NodeId, now: Instant) {
    let Some(receiver) = receiver(node) else {
        return;
    };
    let used = receiver.used.get_untracked();
    let ready = QUEUED.with(|queued| {
        let mut queued = queued.borrow_mut();
        let fits = queued.get(&node).is_some_and(|held| fits(used, held.rect));
        match fits {
            true => queued.remove(&node),
            false => None,
        }
    });
    if let Some(ready) = ready
        && now.saturating_duration_since(ready.since) <= LATE
    {
        deliver(node, ready.events, ready.modifiers, Some(ready.rect));
    }
}

fn fits(used: Option<Vec2>, rect: Rect) -> bool {
    used.is_some_and(|used| {
        (used.x - rect.width()).abs() <= SIZE_SLACK && (used.y - rect.height()).abs() <= SIZE_SLACK
    })
}

fn receiver(node: NodeId) -> Option<Receiver> {
    RECEIVERS.with(|receivers| receivers.borrow().get(&node).copied())
}

fn send(to: NodeId, events: Vec<Forwarded>, modifiers: Modifiers) {
    let waiting = QUEUED.with(|queued| {
        let mut queued = queued.borrow_mut();
        let Some(held) = queued.get_mut(&to) else {
            return Some(events);
        };
        held.events.extend(events);
        held.modifiers = modifiers;
        None
    });
    if let Some(events) = waiting {
        deliver(to, events, modifiers, None);
    }
}

fn deliver(to: NodeId, events: Vec<Forwarded>, modifiers: Modifiers, rect: Option<Rect>) {
    let Some(receiver) = receiver(to) else {
        return;
    };
    let pointer = events.iter().rev().find_map(|forwarded| match forwarded {
        Forwarded::Pointer { event, .. } => Some(event.pos),
        _ => None,
    });
    receiver.forward.call(ForwardedInput {
        events,
        rect: rect.unwrap_or_else(|| receiver.rect.get_untracked()),
        hovered: true,
        focused: false,
        pointer,
        modifiers,
    });
}

#[derive(Clone, Copy)]
struct Held {
    down: PointerEvent,
    last: PointerEvent,
    ended: bool,
    to: Option<NodeId>,
}

impl Held {
    fn replayed(&self) -> Vec<Forwarded> {
        let mut events = vec![Forwarded::Pointer {
            event: self.down,
            over: None,
        }];
        if self.last.pos != self.down.pos {
            events.push(Forwarded::Pointer {
                event: PointerEvent {
                    phase: PointerPhase::Move,
                    ..self.last
                },
                over: None,
            });
        }
        if self.ended {
            events.push(Forwarded::Pointer {
                event: self.last,
                over: None,
            });
        }
        events
    }

    fn cancel(&self) -> Forwarded {
        match self.down.pointer {
            PointerId::Mouse => Forwarded::Left,
            _ => Forwarded::Pointer {
                event: PointerEvent {
                    phase: PointerPhase::Cancel,
                    ..self.last
                },
                over: None,
            },
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Handoff {
    held: Option<Held>,
    rect: Option<Rect>,
}

impl Handoff {
    pub(super) fn route(&mut self, input: ForwardedInput) -> ForwardedInput {
        self.rect = Some(input.rect);
        let mut own = Vec::with_capacity(input.events.len());
        let mut handed = Vec::new();
        let mut to = None;
        for forwarded in input.events {
            let Forwarded::Pointer { event, .. } = &forwarded else {
                own.push(forwarded);
                continue;
            };
            let event = *event;
            if let Some(held) = self.held
                && let Some(receiver) = held.to
                && held.down.pointer == event.pointer
            {
                to = Some(receiver);
                handed.push(Forwarded::Pointer { event, over: None });
                if matches!(
                    event.phase,
                    PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone
                ) {
                    self.held = None;
                }
                continue;
            }
            self.track(event);
            own.push(forwarded);
        }
        if let Some(to) = to {
            send(to, handed, input.modifiers);
        }
        ForwardedInput {
            events: own,
            ..input
        }
    }

    fn track(&mut self, event: PointerEvent) {
        if let PointerPhase::Down(PointerButton::Primary) = event.phase {
            self.held = Some(Held {
                down: event,
                last: event,
                ended: false,
                to: None,
            });
            return;
        }
        let Some(held) = self.held.as_mut() else {
            return;
        };
        if held.down.pointer != event.pointer || held.ended {
            return;
        }
        match event.phase {
            PointerPhase::Move => held.last = event,
            PointerPhase::Up(_) => {
                held.last = event;
                held.ended = true;
            }
            PointerPhase::Cancel | PointerPhase::Gone => self.held = None,
            _ => {}
        }
    }

    pub(super) fn hand_to(
        &mut self,
        to: NodeId,
        within: Rect,
        lands_in: Option<Rect>,
        now: Instant,
    ) -> Option<ForwardedInput> {
        let held = self.held.filter(|held| held.to.is_none())?;
        if !within.contains(held.down.pos)
            || (held.ended && now.saturating_duration_since(held.last.time) > LATE)
        {
            return None;
        }
        self.held = (!held.ended).then_some(Held {
            to: Some(to),
            ..held
        });
        let events = held.replayed();
        let modifiers = held.last.modifiers;
        match lands_in {
            Some(rect) => {
                QUEUED.with(|queued| {
                    queued.borrow_mut().insert(
                        to,
                        Queued {
                            events,
                            modifiers,
                            rect,
                            since: now,
                        },
                    )
                });
                laid_out(to, now);
            }
            None => deliver(to, events, modifiers, None),
        }
        (!held.ended).then(|| ForwardedInput {
            events: vec![held.cancel()],
            rect: self.rect.unwrap_or(Rect::NOTHING),
            hovered: false,
            focused: false,
            pointer: None,
            modifiers,
        })
    }
}
