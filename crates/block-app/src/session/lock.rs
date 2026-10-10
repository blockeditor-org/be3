use std::time::Duration;

pub(crate) use crate::password::Password;

pub(crate) const FREE_ATTEMPTS: u32 = 3;
pub(crate) const FIRST_WAIT: Duration = Duration::from_secs(5);
pub(crate) const LONGEST_WAIT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trigger {
    Desktop,
    Logind,
    Sleep,
}

impl Trigger {
    fn reason(self) -> &'static str {
        match self {
            Self::Desktop => "the desktop",
            Self::Logind => "logind",
            Self::Sleep => "a suspend",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    Accepted,
    Denied(String),
    Failed(String),
}

pub(crate) trait Authenticate {
    fn start(&mut self, attempt: u64, password: Password);
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LockState {
    pub(crate) locked: bool,
    pub(crate) checking: bool,
    pub(crate) retry_in: Option<Duration>,
    pub(crate) error: Option<String>,
}

impl LockState {
    pub(crate) fn busy(&self) -> bool {
        self.checking || self.retry_in.is_some()
    }

    pub(crate) fn retry_in_seconds(&self) -> Option<u32> {
        let wait = self.retry_in?;
        let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
        Some(u32::try_from(seconds).unwrap_or(u32::MAX))
    }

    pub(crate) fn message(&self) -> Option<String> {
        match self.retry_in_seconds() {
            Some(1) => Some("Too many attempts. Try again in 1 second.".to_owned()),
            Some(seconds) => Some(format!(
                "Too many attempts. Try again in {seconds} seconds."
            )),
            None => self.error.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Lock {
    locked: bool,
    attempt: u64,
    checking: bool,
    failures: u32,
    retry_at: Option<Duration>,
    error: Option<String>,
}

impl Lock {
    pub(crate) fn locked(&self) -> bool {
        self.locked
    }

    pub(crate) fn lock(&mut self, trigger: Trigger) -> bool {
        if self.locked {
            return false;
        }
        eprintln!("block-app: locking the screen, for {}", trigger.reason());
        self.locked = true;
        self.attempt += 1;
        self.checking = false;
        self.error = None;
        true
    }

    pub(crate) fn submit(
        &mut self,
        password: Password,
        now: Duration,
        authenticate: &mut impl Authenticate,
    ) -> Option<Duration> {
        if !self.locked || self.checking || password.as_str().is_empty() {
            return None;
        }
        if let Some(wait) = self.wait(now) {
            return Some(wait);
        }
        self.attempt += 1;
        self.checking = true;
        self.error = None;
        authenticate.start(self.attempt, password);
        None
    }

    pub(crate) fn answered(&mut self, attempt: u64, verdict: Verdict, now: Duration) -> bool {
        if !self.locked || !self.checking || attempt != self.attempt {
            return false;
        }
        self.checking = false;
        match verdict {
            Verdict::Accepted => {
                eprintln!("block-app: unlocked the screen");
                self.locked = false;
                self.failures = 0;
                self.retry_at = None;
                self.error = None;
                self.attempt += 1;
                return true;
            }
            Verdict::Denied(reason) => {
                self.failures += 1;
                self.retry_at = penalty(self.failures).map(|wait| now + wait);
                self.error = Some(reason);
            }
            Verdict::Failed(problem) => {
                eprintln!("block-app: the password could not be checked: {problem}");
                self.error = Some(format!("The password could not be checked: {problem}"));
            }
        }
        false
    }

    pub(crate) fn wait(&self, now: Duration) -> Option<Duration> {
        self.retry_at
            .filter(|retry| *retry > now)
            .map(|retry| retry - now)
    }

    pub(crate) fn state(&self, now: Duration) -> LockState {
        LockState {
            locked: self.locked,
            checking: self.checking,
            retry_in: self.wait(now).filter(|_| self.locked),
            error: self.error.clone().filter(|_| self.locked),
        }
    }
}

fn penalty(failures: u32) -> Option<Duration> {
    let over = failures.checked_sub(FREE_ATTEMPTS)?;
    let doubled = FIRST_WAIT.saturating_mul(1 << over.min(16));
    Some(doubled.min(LONGEST_WAIT))
}

#[cfg(test)]
mod tests;
