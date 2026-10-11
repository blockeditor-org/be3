use std::time::Duration;

pub(crate) const FLIP_GRACE: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lane {
    pub(crate) busy: bool,
    pub(crate) flipped: bool,
    pub(crate) refresh: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wait {
    Now,
    Until(Duration),
    Idle,
}

#[derive(Default)]
pub(crate) struct Pacer {
    asked: bool,
    due: Option<Duration>,
    last: Option<Duration>,
}

impl Pacer {
    pub(crate) fn ask(&mut self) {
        self.asked = true;
    }

    pub(crate) fn ran(&mut self, now: Duration, again: bool, after: Option<Duration>) {
        self.last = Some(now);
        self.asked = again;
        self.due = after.map(|after| now.saturating_add(after));
    }

    pub(crate) fn wait(&self, now: Duration, lanes: &[Lane]) -> Wait {
        let wanted = match (self.asked, self.due) {
            (true, _) => now,
            (false, Some(due)) => due,
            (false, None) => return Wait::Idle,
        };
        let at = wanted.max(self.earliest(lanes));
        if at <= now {
            Wait::Now
        } else {
            Wait::Until(at)
        }
    }

    fn earliest(&self, lanes: &[Lane]) -> Duration {
        let Some(last) = self.last else {
            return Duration::ZERO;
        };
        if lanes.iter().any(|lane| lane.flipped) {
            return Duration::ZERO;
        }
        if lanes.iter().any(|lane| lane.busy) {
            return last.saturating_add(FLIP_GRACE);
        }
        lanes
            .iter()
            .map(|lane| lane.refresh)
            .filter(|refresh| !refresh.is_zero())
            .min()
            .map_or(Duration::ZERO, |refresh| last.saturating_add(refresh))
    }
}

pub(crate) fn refresh_interval(millihertz: u32) -> Duration {
    match millihertz {
        0 => Duration::ZERO,
        millihertz => Duration::from_nanos(1_000_000_000_000 / u64::from(millihertz)),
    }
}

#[cfg(test)]
mod tests;
