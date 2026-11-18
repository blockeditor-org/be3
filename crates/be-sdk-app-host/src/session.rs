//! An app's lifecycle as its host sees it: starting, running, shutting down, and failing when
//! it takes too long at either end.
use be_sdk_api::{Hello, HelloAccepted, Preferences, SurfaceSpec, SurfaceSupport};

const LIFECYCLE_TIMEOUT_MILLISECONDS: u64 = 5_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Starting,
    Running,
    ShuttingDown,
    Closed,
    Failed(SessionFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionFailure {
    Protocol(String),
    StartupTimedOut,
    ShutdownTimedOut,
}

pub struct Session {
    state: SessionState,
    host_name: String,
    surface: Option<SurfaceSpec>,
    granted: Option<SurfaceSpec>,
    deadline: Option<u64>,
}

impl Session {
    pub fn new(host_name: impl Into<String>, surface: Option<SurfaceSpec>) -> Self {
        Self {
            state: SessionState::Idle,
            host_name: host_name.into(),
            surface,
            granted: None,
            deadline: None,
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn running(&self) -> bool {
        self.state == SessionState::Running
    }

    pub fn granted_surface(&self) -> Option<SurfaceSpec> {
        self.granted
    }

    pub fn start(&mut self, now_milliseconds: u64) {
        self.granted = None;
        self.state = SessionState::Starting;
        self.deadline = Some(now_milliseconds + LIFECYCLE_TIMEOUT_MILLISECONDS);
    }

    pub fn connect(&mut self, hello: &Hello, preferences: Preferences) -> HelloAccepted {
        if self.state != SessionState::Starting {
            self.fail(SessionFailure::Protocol(
                "the app connected twice".to_owned(),
            ));
        } else {
            self.state = SessionState::Running;
            self.deadline = None;
        }
        self.granted = match hello.surface {
            SurfaceSupport::Texture => self.surface,
            SurfaceSupport::None => None,
        };
        HelloAccepted {
            host_name: self.host_name.clone(),
            surface: self.granted,
            preferences,
        }
    }

    pub fn shutdown(&mut self, now_milliseconds: u64) -> bool {
        match self.state {
            SessionState::Idle | SessionState::Closed | SessionState::Failed(_) => {
                self.state = SessionState::Closed;
                self.deadline = None;
                false
            }
            SessionState::Starting | SessionState::Running => {
                self.state = SessionState::ShuttingDown;
                self.deadline = Some(now_milliseconds + LIFECYCLE_TIMEOUT_MILLISECONDS);
                true
            }
            SessionState::ShuttingDown => false,
        }
    }

    pub fn shut_down(&mut self) {
        match self.state {
            SessionState::ShuttingDown => {
                self.state = SessionState::Closed;
                self.deadline = None;
            }
            SessionState::Failed(_) | SessionState::Closed => {}
            _ => self.fail(SessionFailure::Protocol(
                "the app shut down without being asked".to_owned(),
            )),
        }
    }

    pub fn fail(&mut self, failure: SessionFailure) {
        self.state = SessionState::Failed(failure);
        self.deadline = None;
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.deadline
    }

    pub fn tick(&mut self, now_milliseconds: u64) {
        if self
            .deadline
            .is_some_and(|deadline| now_milliseconds >= deadline)
        {
            let failure = match self.state {
                SessionState::Starting => SessionFailure::StartupTimedOut,
                SessionState::ShuttingDown => SessionFailure::ShutdownTimedOut,
                _ => return,
            };
            self.fail(failure);
        }
    }
}
