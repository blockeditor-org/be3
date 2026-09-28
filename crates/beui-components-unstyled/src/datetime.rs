use std::time::{SystemTime, UNIX_EPOCH};

pub const SECONDS_PER_DAY: i64 = 86_400;
pub const MINUTES_PER_DAY: u32 = 24 * 60;

pub const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub const ALL: [Weekday; 7] = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];

    pub fn from_monday(index: u32) -> Self {
        Self::ALL[(index % 7) as usize]
    }

    pub fn days_from_monday(self) -> u32 {
        self as u32
    }

    pub fn offset(self, days: u32) -> Self {
        Self::from_monday(self.days_from_monday() + days)
    }

    pub fn name(self) -> &'static str {
        match self {
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
            Weekday::Sunday => "Sunday",
        }
    }

    pub fn short_name(self) -> &'static str {
        &self.name()[..3]
    }

    pub fn narrow_name(self) -> &'static str {
        &self.name()[..2]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub const MIN_YEAR: i32 = 1;
    pub const MAX_YEAR: i32 = 9999;
    pub const EPOCH: Date = Date {
        year: 1970,
        month: 1,
        day: 1,
    };

    pub fn new(year: i32, month: u8, day: u8) -> Self {
        let year = year.clamp(Self::MIN_YEAR, Self::MAX_YEAR);
        let month = month.clamp(1, 12);
        Self {
            year,
            month,
            day: day.clamp(1, days_in_month(year, month)),
        }
    }

    pub fn from_days(days_since_epoch: i64) -> Self {
        let days = days_since_epoch + 719_468;
        let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
        let day_of_era = days - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_prime = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
        let month = month_prime + if month_prime < 10 { 3 } else { -9 };
        let year = year_of_era as i32 + era as i32 * 400 + i32::from(month <= 2);
        Self {
            year,
            month: month as u8,
            day: day as u8,
        }
    }

    pub fn days(self) -> i64 {
        let year = self.year - i32::from(self.month <= 2);
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month = i32::from(self.month);
        let day_of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i32::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        i64::from(era) * 146_097 + i64::from(day_of_era) - 719_468
    }

    pub fn today() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0);
        Self::from_days(seconds.div_euclid(SECONDS_PER_DAY))
    }

    pub fn weekday(self) -> Weekday {
        Weekday::from_monday((self.days() + 3).rem_euclid(7) as u32)
    }

    pub fn add_days(self, days: i64) -> Self {
        Self::from_days(self.days() + days)
    }

    pub fn add_months(self, months: i32) -> Self {
        let index = self.year * 12 + i32::from(self.month) - 1 + months;
        Self::new(
            index.div_euclid(12),
            (index.rem_euclid(12) + 1) as u8,
            self.day,
        )
    }

    pub fn add_years(self, years: i32) -> Self {
        Self::new(self.year + years, self.month, self.day)
    }

    pub fn first_of_month(self) -> Self {
        Self { day: 1, ..self }
    }

    pub fn last_of_month(self) -> Self {
        Self {
            day: days_in_month(self.year, self.month),
            ..self
        }
    }

    pub fn same_month(self, other: Date) -> bool {
        self.year == other.year && self.month == other.month
    }

    pub fn start_of_week(self, first: Weekday) -> Self {
        let back = (self.weekday().days_from_monday() + 7 - first.days_from_monday()) % 7;
        self.add_days(-i64::from(back))
    }

    pub fn clamp_to(self, min: Option<Date>, max: Option<Date>) -> Self {
        let raised = min.map_or(self, |min| self.max(min));
        max.map_or(raised, |max| raised.min(max))
    }

    pub fn month_name(self) -> &'static str {
        MONTH_NAMES[usize::from(self.month.clamp(1, 12) - 1)]
    }

    pub fn month_label(self) -> String {
        format!("{} {}", self.month_name(), self.year)
    }

    pub fn label(self) -> String {
        format!(
            "{}, {} {} {}",
            self.weekday().name(),
            self.day,
            self.month_name(),
            self.year
        )
    }

    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Default for Date {
    fn default() -> Self {
        Self::EPOCH
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
}

impl Time {
    pub const MIDNIGHT: Time = Time { hour: 0, minute: 0 };

    pub fn new(hour: u8, minute: u8) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }

    pub fn from_minutes(minutes: u32) -> Self {
        let minutes = minutes % MINUTES_PER_DAY;
        Self {
            hour: (minutes / 60) as u8,
            minute: (minutes % 60) as u8,
        }
    }

    pub fn minutes(self) -> u32 {
        u32::from(self.hour) * 60 + u32::from(self.minute)
    }

    pub fn now() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0);
        Self::from_minutes((seconds.rem_euclid(SECONDS_PER_DAY) / 60) as u32)
    }

    pub fn format(self, cycle: HourCycle) -> String {
        match cycle {
            HourCycle::H24 => format!("{:02}:{:02}", self.hour, self.minute),
            HourCycle::H12 => format!(
                "{}:{:02} {}",
                twelve_hour(self.hour),
                self.minute,
                if self.hour < 12 { "AM" } else { "PM" }
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}

impl DateTime {
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }

    pub fn from_unix(seconds: i64) -> Self {
        Self {
            date: Date::from_days(seconds.div_euclid(SECONDS_PER_DAY)),
            time: Time::from_minutes((seconds.rem_euclid(SECONDS_PER_DAY) / 60) as u32),
        }
    }

    pub fn to_unix(self) -> i64 {
        self.date.days() * SECONDS_PER_DAY + i64::from(self.time.minutes()) * 60
    }

    pub fn now() -> Self {
        Self::new(Date::today(), Time::now())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum HourCycle {
    H12,
    #[default]
    H24,
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

pub fn twelve_hour(hour: u8) -> u8 {
    match hour % 12 {
        0 => 12,
        hour => hour,
    }
}

#[cfg(test)]
mod tests;
