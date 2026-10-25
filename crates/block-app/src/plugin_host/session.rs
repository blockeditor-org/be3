use block_plugin_api::{Hello, HelloAccepted, Preferences, SurfaceSpec, SurfaceSupport};

/// How long a plugin has to say hello, or to finish shutting down.
const LIFECYCLE_TIMEOUT_MILLISECONDS: u64 = 5_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SessionState {
    Idle,
    Starting,
    Running,
    ShuttingDown,
    Closed,
    Failed(SessionFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SessionFailure {
    Protocol(String),
    StartupTimedOut,
    ShutdownTimedOut,
}

/// Where the host's side of a plugin is in its life: started, connected (its
/// `connect` answered with the surface it may draw on), shutting down, closed.
pub(super) struct Session {
    state: SessionState,
    host_name: String,
    surface: Option<SurfaceSpec>,
    granted: Option<SurfaceSpec>,
    deadline: Option<u64>,
}

impl Session {
    pub(super) fn new(host_name: impl Into<String>, surface: Option<SurfaceSpec>) -> Self {
        Self {
            state: SessionState::Idle,
            host_name: host_name.into(),
            surface,
            granted: None,
            deadline: None,
        }
    }

    pub(super) fn state(&self) -> &SessionState {
        &self.state
    }

    pub(super) fn running(&self) -> bool {
        self.state == SessionState::Running
    }

    pub(super) fn granted_surface(&self) -> Option<SurfaceSpec> {
        self.granted
    }

    pub(super) fn start(&mut self, now_milliseconds: u64) {
        self.granted = None;
        self.state = SessionState::Starting;
        self.deadline = Some(now_milliseconds + LIFECYCLE_TIMEOUT_MILLISECONDS);
    }

    /// The answer to the plugin's `connect`, which starts the session.
    pub(super) fn connect(&mut self, hello: &Hello, preferences: Preferences) -> HelloAccepted {
        if self.state != SessionState::Starting {
            self.fail(SessionFailure::Protocol(
                "the plugin connected twice".to_owned(),
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

    /// Whether the plugin is to be asked to shut down.
    pub(super) fn shutdown(&mut self, now_milliseconds: u64) -> bool {
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

    pub(super) fn shut_down(&mut self) {
        match self.state {
            SessionState::ShuttingDown => {
                self.state = SessionState::Closed;
                self.deadline = None;
            }
            SessionState::Failed(_) | SessionState::Closed => {}
            _ => self.fail(SessionFailure::Protocol(
                "the plugin shut down without being asked".to_owned(),
            )),
        }
    }

    pub(super) fn fail(&mut self, failure: SessionFailure) {
        self.state = SessionState::Failed(failure);
        self.deadline = None;
    }

    pub(super) fn next_deadline(&self) -> Option<u64> {
        self.deadline
    }

    pub(super) fn tick(&mut self, now_milliseconds: u64) {
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
