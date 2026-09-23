use std::time::{SystemTime, UNIX_EPOCH};

use block_editor_plugin::be_block::{CalendarEvent, Item, ObjectId};
pub(crate) use block_ui::datetime::SECONDS_PER_DAY;
use block_ui::datetime::{DateTimeFields, MONTH_NAMES, civil_from_days, days_from_civil};

pub(crate) const WEEKDAY_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
pub(crate) const WEEKDAY_FULL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CalendarView {
    Day,
    Week,
    #[default]
    Month,
}

impl CalendarView {
    pub(crate) const ALL: [Self; 3] = [Self::Day, Self::Week, Self::Month];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Day => "Day",
            Self::Week => "Week",
            Self::Month => "Month",
        }
    }

    pub(crate) fn previous(self, anchor: i64) -> i64 {
        match self {
            Self::Day => anchor - 1,
            Self::Week => anchor - 7,
            Self::Month => {
                let (year, month, _) = civil_from_days(anchor);
                let (year, month) = match month == 1 {
                    true => (year - 1, 12),
                    false => (year, month - 1),
                };
                days_from_civil(year, month, 1)
            }
        }
    }

    pub(crate) fn next(self, anchor: i64) -> i64 {
        match self {
            Self::Day => anchor + 1,
            Self::Week => anchor + 7,
            Self::Month => {
                let (year, month, _) = civil_from_days(anchor);
                let (year, month) = match month == 12 {
                    true => (year + 1, 1),
                    false => (year, month + 1),
                };
                days_from_civil(year, month, 1)
            }
        }
    }

    pub(crate) fn header(self, anchor: i64) -> String {
        match self {
            Self::Day => {
                let (year, month, day) = civil_from_days(anchor);
                format!(
                    "{}, {} {}, {}",
                    WEEKDAY_FULL[weekday_from_days(anchor) as usize],
                    MONTH_NAMES[(month - 1) as usize],
                    day,
                    year
                )
            }
            Self::Week => {
                let start = week_start(anchor);
                let end = start + 6;
                let (start_year, start_month, start_day) = civil_from_days(start);
                let (end_year, end_month, end_day) = civil_from_days(end);
                match start_year == end_year && start_month == end_month {
                    true => format!(
                        "{} {} - {}, {}",
                        MONTH_NAMES[(start_month - 1) as usize],
                        start_day,
                        end_day,
                        start_year
                    ),
                    false => format!(
                        "{} {} - {} {}, {}",
                        MONTH_NAMES[(start_month - 1) as usize],
                        start_day,
                        MONTH_NAMES[(end_month - 1) as usize],
                        end_day,
                        end_year
                    ),
                }
            }
            Self::Month => {
                let (year, month, _) = civil_from_days(anchor);
                format!("{} {}", MONTH_NAMES[(month - 1) as usize], year)
            }
        }
    }
}

pub(crate) type Shown = Item<CalendarEvent>;

#[derive(Clone)]
pub(crate) enum FormAction {
    Save(Option<ObjectId>, CalendarEvent),
    Delete(ObjectId),
}

#[derive(Clone, PartialEq)]
pub(crate) struct EventForm {
    pub(crate) editing_id: Option<ObjectId>,
    pub(crate) title: String,
    pub(crate) start: DateTimeFields,
    pub(crate) end: DateTimeFields,
}

impl EventForm {
    pub(crate) fn new_at(day: i64, hour: u8) -> Self {
        let start = day * SECONDS_PER_DAY + i64::from(hour) * 3600;
        Self {
            editing_id: None,
            title: String::new(),
            start: DateTimeFields::from_unix(start),
            end: DateTimeFields::from_unix(start + 3600),
        }
    }

    pub(crate) fn edit(event: &Shown) -> Self {
        Self {
            editing_id: Some(event.id),
            title: event.title.clone(),
            start: DateTimeFields::from_unix(event.start),
            end: DateTimeFields::from_unix(event.end),
        }
    }

    pub(crate) fn event(&self) -> CalendarEvent {
        let start = self.start.to_unix();
        CalendarEvent::new(self.title.trim(), start, self.end.to_unix())
    }
}

pub(crate) fn assign_lanes(events: &[Shown]) -> (Vec<usize>, usize) {
    let mut lane_ends: Vec<i64> = Vec::new();
    let mut lanes = Vec::with_capacity(events.len());
    for event in events {
        match lane_ends.iter().position(|end| *end <= event.start) {
            Some(lane) => {
                lane_ends[lane] = event.end;
                lanes.push(lane);
            }
            None => {
                lanes.push(lane_ends.len());
                lane_ends.push(event.end);
            }
        }
    }
    (lanes, lane_ends.len().max(1))
}

pub(crate) fn events_on(events: &[Shown], day: i64) -> Vec<Shown> {
    let start = day * SECONDS_PER_DAY;
    let end = start + SECONDS_PER_DAY;
    let mut day_events: Vec<_> = events
        .iter()
        .filter(|event| event.start < end && event.end > start)
        .cloned()
        .collect();
    day_events.sort_by_key(|event| event.start);
    day_events
}

pub(crate) fn week_start(day: i64) -> i64 {
    day - i64::from(weekday_from_days(day))
}

pub(crate) fn today_days_since_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| (duration.as_secs() / SECONDS_PER_DAY as u64) as i64)
        .unwrap_or(0)
}

pub(crate) fn weekday_from_days(days_since_epoch: i64) -> u8 {
    (days_since_epoch + 4).rem_euclid(7) as u8
}
