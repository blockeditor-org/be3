use stable_hash::HashMap;
use std::time::Duration;

pub(crate) const PREVIEW_COOLDOWN: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
enum Held {
    Settling { until: Duration },
    Painted { at: Duration },
}

#[derive(Default)]
pub(crate) struct Cooldown {
    held: HashMap<u32, Held>,
}

impl Cooldown {
    pub(crate) fn keep(&mut self, kept: impl Fn(u32) -> bool) {
        self.held.retain(|surface, _| kept(*surface));
    }

    pub(crate) fn wait(&self, surface: u32, now: Duration) -> Option<Duration> {
        match self.held.get(&surface)? {
            Held::Settling { .. } => None,
            Held::Painted { at } => (*at + PREVIEW_COOLDOWN)
                .checked_sub(now)
                .filter(|wait| !wait.is_zero()),
        }
    }

    pub(crate) fn painted(&mut self, surface: u32, now: Duration, fresh: bool) {
        let held = match (fresh, self.held.get(&surface)) {
            (true, _) => Held::Settling {
                until: now + PREVIEW_COOLDOWN,
            },
            (false, Some(Held::Settling { until })) if now < *until => {
                Held::Settling { until: *until }
            }
            (false, _) => Held::Painted { at: now },
        };
        self.held.insert(surface, held);
    }
}

#[cfg(test)]
mod tests;
