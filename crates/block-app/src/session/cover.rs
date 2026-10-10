use std::time::Duration;

pub(crate) const DRAW_WITHIN: Duration = Duration::from_secs(1);
pub(crate) const HUNG_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Cover {
    #[default]
    Blank,
    Plugin,
    Fallback,
}

impl Cover {
    pub(crate) fn takes_desktop_passwords(self) -> bool {
        self == Self::Plugin
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginLock {
    pub(crate) offered: bool,
    pub(crate) failed: bool,
    pub(crate) drawn: bool,
    pub(crate) unanswered: Option<Duration>,
}

#[derive(Debug, Default)]
pub(crate) struct CoverChoice {
    since: Option<Duration>,
    fallen_back: bool,
}

impl CoverChoice {
    pub(crate) fn choose(
        &mut self,
        locked: bool,
        now: Duration,
        plugin: PluginLock,
    ) -> (Cover, Option<Duration>) {
        if !locked {
            *self = Self::default();
            return (Cover::Blank, None);
        }
        let since = *self.since.get_or_insert(now);
        let waited = now.saturating_sub(since);
        let hung = plugin
            .unanswered
            .is_some_and(|unanswered| unanswered >= HUNG_AFTER);
        let problem = if !plugin.offered {
            Some("the desktop has no lock screen")
        } else if plugin.failed {
            Some("the desktop stopped")
        } else if hung {
            Some("the desktop stopped answering")
        } else if !plugin.drawn && waited >= DRAW_WITHIN {
            Some("the desktop did not draw its lock screen in time")
        } else {
            None
        };
        if let Some(problem) = problem
            && !self.fallen_back
        {
            eprintln!("block-app: showing the built-in lock screen, since {problem}");
            self.fallen_back = true;
        }
        if self.fallen_back {
            return (Cover::Fallback, None);
        }
        let until_hung = plugin
            .unanswered
            .map(|unanswered| HUNG_AFTER - unanswered);
        match plugin.drawn {
            true => (Cover::Plugin, until_hung),
            false => {
                let until_late = DRAW_WITHIN - waited;
                let wake = until_hung.map_or(until_late, |until_hung| until_hung.min(until_late));
                (Cover::Blank, Some(wake))
            }
        }
    }
}

#[cfg(test)]
mod tests;
