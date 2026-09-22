use be_commit::MergeResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    BlockContent, ContentError, LiveEdit, Merge, Undo,
    keyed::{merge_keyed, pick},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CalendarEvent {
    pub id: Uuid,
    pub title: String,
    pub start: i64,
    pub end: i64,
}

impl CalendarEvent {
    pub fn new(title: String, start: i64, end: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            start,
            end,
        }
    }

    fn normalized(mut self) -> Self {
        if self.end < self.start {
            self.end = self.start;
        }
        self
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CalendarContent {
    events: Vec<CalendarEvent>,
}

impl CalendarContent {
    pub fn events(&self) -> &[CalendarEvent] {
        &self.events
    }

    pub fn event(&self, id: Uuid) -> Option<&CalendarEvent> {
        self.events.iter().find(|event| event.id == id)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CalendarOp {
    AddEvent { event: CalendarEvent },
    UpdateEvent { event: CalendarEvent },
    RemoveEvent { id: Uuid },
}

impl BlockContent for CalendarContent {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6361_6c65_6e64_6172_2d62_6c6f_636b_0002);

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("calendar"))
    }
}

impl LiveEdit for CalendarContent {
    type Op = CalendarOp;

    fn apply(&mut self, operation: &Self::Op) {
        match operation {
            CalendarOp::AddEvent { event } => {
                if self.event(event.id).is_none() {
                    self.events.push(event.clone().normalized());
                }
            }
            CalendarOp::UpdateEvent { event } => {
                if let Some(existing) = self.events.iter_mut().find(|held| held.id == event.id) {
                    *existing = event.clone().normalized();
                }
            }
            CalendarOp::RemoveEvent { id } => self.events.retain(|event| event.id != *id),
        }
    }
}

impl Merge for CalendarContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        let merged = merge_keyed(
            &base.events,
            &ours.events,
            &theirs.events,
            |event| event.id,
            |base, ours, theirs, conflicts| {
                CalendarEvent {
                    id: base.id,
                    title: pick(&base.title, &ours.title, &theirs.title, conflicts),
                    start: pick(&base.start, &ours.start, &theirs.start, conflicts),
                    end: pick(&base.end, &ours.end, &theirs.end, conflicts),
                }
                .normalized()
            },
        );
        match merged {
            MergeResult::Clean(events) => MergeResult::Clean(Self { events }),
            MergeResult::Conflicted { value, conflicts } => MergeResult::Conflicted {
                value: Self { events: value },
                conflicts,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalendarStep {
    Added(CalendarEvent),
    Removed(CalendarEvent),
    Changed {
        before: CalendarEvent,
        after: CalendarEvent,
    },
}

impl Undo for CalendarContent {
    type Step = CalendarStep;

    fn step(&self, operation: &Self::Op) -> Option<Self::Step> {
        match operation {
            CalendarOp::AddEvent { event } => (self.event(event.id).is_none())
                .then(|| CalendarStep::Added(event.clone().normalized())),
            CalendarOp::RemoveEvent { id } => self.event(*id).cloned().map(CalendarStep::Removed),
            CalendarOp::UpdateEvent { event } => {
                let before = self.event(event.id)?.clone();
                let after = event.clone().normalized();
                (before != after).then_some(CalendarStep::Changed { before, after })
            }
        }
    }

    fn absorb(previous: &mut Self::Step, next: Self::Step) -> Result<(), Self::Step> {
        match (previous, next) {
            (
                CalendarStep::Changed { after, .. },
                CalendarStep::Changed {
                    after: latest,
                    before,
                },
            ) if after.id == latest.id && *after == before => {
                *after = latest;
                Ok(())
            }
            (_, next) => Err(next),
        }
    }

    fn revert(&self, step: &Self::Step) -> Vec<Self::Op> {
        match step {
            CalendarStep::Added(event) => vec![CalendarOp::RemoveEvent { id: event.id }],
            CalendarStep::Removed(event) => vec![CalendarOp::AddEvent {
                event: event.clone(),
            }],
            CalendarStep::Changed { before, after } => self.move_event(after, before),
        }
    }

    fn reapply(&self, step: &Self::Step) -> Vec<Self::Op> {
        match step {
            CalendarStep::Added(event) => vec![CalendarOp::AddEvent {
                event: event.clone(),
            }],
            CalendarStep::Removed(event) => vec![CalendarOp::RemoveEvent { id: event.id }],
            CalendarStep::Changed { before, after } => self.move_event(before, after),
        }
    }
}

impl CalendarContent {
    fn move_event(&self, expected: &CalendarEvent, desired: &CalendarEvent) -> Vec<CalendarOp> {
        let Some(current) = self.event(expected.id) else {
            return Vec::new();
        };
        let mut event = current.clone();
        if event.title == expected.title {
            event.title.clone_from(&desired.title);
        }
        if event.start == expected.start {
            event.start = desired.start;
        }
        if event.end == expected.end {
            event.end = desired.end;
        }
        vec![CalendarOp::UpdateEvent { event }]
    }
}
