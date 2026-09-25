use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Calendar {
    pub events: List<CalendarEvent>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct CalendarEvent {
    pub title: String,
    pub start: i64,
    pub end: i64,
}

impl CalendarEvent {
    pub fn new(title: impl Into<String>, start: i64, end: i64) -> Self {
        Self {
            title: title.into(),
            start,
            end: end.max(start),
        }
    }

    pub fn ends(&self) -> i64 {
        self.end.max(self.start)
    }
}

impl Calendar {
    pub fn add(event: &CalendarEvent) -> (ObjectId, Edit) {
        let (id, change) = Self::EVENTS.insert(ObjectId::ROOT, Anchor::End, event);
        (id, change.into())
    }

    pub fn update(&self, id: ObjectId, event: &CalendarEvent) -> Edit {
        let Some(current) = self.events.get(id) else {
            return Edit::default();
        };
        let mut changes = Vec::new();
        if current.title != event.title {
            changes.push(CalendarEvent::TITLE.set(id, &event.title));
        }
        if current.start != event.start {
            changes.push(CalendarEvent::START.set(id, &event.start));
        }
        if current.end != event.end {
            changes.push(CalendarEvent::END.set(id, &event.end));
        }
        Edit(changes)
    }

    pub fn remove(id: ObjectId) -> Edit {
        Change::remove(id).into()
    }
}

impl Root for Calendar {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6361_6c65_6e64_6172_2d62_6c6f_636b_0001);
}

pub type CalendarContent = Document<Calendar>;
