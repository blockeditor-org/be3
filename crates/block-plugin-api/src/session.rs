use crate::{
    DecodeError, HelloAccepted, InputBatch, InputEvent, MAX_QUEUED_MESSAGES, Message,
    REQUEST_TIMEOUT_MILLISECONDS, SurfaceSpec, SurfaceSupport, decode_frame,
};
use std::collections::{HashMap, VecDeque};

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
    MalformedMessage,
    Protocol(String),
    RequestTimedOut(u64),
    StartupTimedOut,
    ShutdownTimedOut,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueError {
    NotRunning,
    Full,
    DuplicateRequest,
}

pub struct HostSession {
    state: SessionState,
    host_name: String,
    preferences: crate::Preferences,
    surface: Option<SurfaceSpec>,
    granted: Option<SurfaceSpec>,
    queue: VecDeque<Message>,
    requests: HashMap<u64, u64>,
    lifecycle_deadline: Option<u64>,
}

impl HostSession {
    pub fn new(
        host_name: impl Into<String>,
        surface: Option<SurfaceSpec>,
        preferences: crate::Preferences,
    ) -> Self {
        Self {
            state: SessionState::Idle,
            host_name: host_name.into(),
            preferences,
            surface,
            granted: None,
            queue: VecDeque::new(),
            requests: HashMap::new(),
            lifecycle_deadline: None,
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn granted_surface(&self) -> Option<SurfaceSpec> {
        self.granted
    }

    pub fn queued_message_count(&self) -> usize {
        self.queue.len()
    }

    pub fn pending_request_count(&self) -> usize {
        self.requests.len()
    }

    pub fn start(&mut self, now_milliseconds: u64) {
        self.queue.clear();
        self.requests.clear();
        self.granted = None;
        self.state = SessionState::Starting;
        self.lifecycle_deadline = Some(now_milliseconds + REQUEST_TIMEOUT_MILLISECONDS);
    }

    pub fn receive_frame(&mut self, frame: &[u8], now_milliseconds: u64) {
        match decode_frame(frame) {
            Ok(message) => self.receive(message, now_milliseconds),
            Err(error) => self.fail(decode_failure(error)),
        }
    }

    pub fn receive(&mut self, message: Message, now_milliseconds: u64) {
        match (&self.state, message) {
            (SessionState::Starting, Message::Hello(hello)) => {
                let granted = match hello.surface {
                    SurfaceSupport::Texture => self.surface,
                    SurfaceSupport::None => None,
                };
                self.granted = granted;
                self.queue.push_back(Message::HelloAccepted(HelloAccepted {
                    host_name: self.host_name.clone(),
                    surface: granted,
                    preferences: self.preferences,
                }));
                self.state = SessionState::Running;
                self.lifecycle_deadline = None;
            }
            (SessionState::Running, Message::Acknowledged { request_id }) => {
                if self.requests.remove(&request_id).is_none() {
                    self.fail(SessionFailure::Protocol(format!(
                        "acknowledgement for unknown request {request_id}"
                    )));
                }
            }
            (SessionState::Running, Message::Error(error)) => {
                if let Some(request_id) = error.request_id {
                    self.requests.remove(&request_id);
                }
                self.fail(SessionFailure::Protocol(error.message));
            }
            (SessionState::ShuttingDown, Message::ShutdownAcknowledged) => {
                self.state = SessionState::Closed;
                self.lifecycle_deadline = None;
                self.requests.clear();
            }
            (SessionState::Failed(_), _) | (SessionState::Closed, _) => {}
            _ => self.fail(SessionFailure::Protocol(
                "message is invalid in the current session state".into(),
            )),
        }
        self.tick(now_milliseconds);
    }

    pub fn send(&mut self, message: Message, now_milliseconds: u64) -> Result<(), QueueError> {
        match &message {
            Message::Screens(set) => {
                let request_id = set.request_id;
                self.enqueue_request(request_id, message, now_milliseconds)
            }
            _ => self.enqueue(message),
        }
    }

    pub fn enqueue(&mut self, message: Message) -> Result<(), QueueError> {
        if self.state != SessionState::Running {
            return Err(QueueError::NotRunning);
        }
        let superseded = superseded_request(&self.queue, &message);
        if coalesce(&mut self.queue, &message) {
            if let Some(request_id) = superseded {
                self.requests.remove(&request_id);
            }
            return Ok(());
        }
        if self.queue.len() == MAX_QUEUED_MESSAGES {
            return Err(QueueError::Full);
        }
        self.queue.push_back(message);
        Ok(())
    }

    pub fn enqueue_request(
        &mut self,
        request_id: u64,
        message: Message,
        now_milliseconds: u64,
    ) -> Result<(), QueueError> {
        if self.requests.contains_key(&request_id) {
            return Err(QueueError::DuplicateRequest);
        }
        self.enqueue(message)?;
        self.requests
            .insert(request_id, now_milliseconds + REQUEST_TIMEOUT_MILLISECONDS);
        Ok(())
    }

    pub fn next_outbound(&mut self) -> Option<Message> {
        self.queue.pop_front()
    }

    pub fn shutdown(&mut self, now_milliseconds: u64) {
        match self.state {
            SessionState::Idle | SessionState::Closed | SessionState::Failed(_) => {
                self.state = SessionState::Closed;
                self.lifecycle_deadline = None;
            }
            SessionState::Starting | SessionState::Running => {
                self.queue.clear();
                self.queue.push_back(Message::Shutdown);
                self.state = SessionState::ShuttingDown;
                self.lifecycle_deadline = Some(now_milliseconds + REQUEST_TIMEOUT_MILLISECONDS);
            }
            SessionState::ShuttingDown => {}
        }
    }

    pub fn disconnected(&mut self) {
        if !matches!(self.state, SessionState::Closed | SessionState::Failed(_)) {
            self.fail(SessionFailure::Disconnected);
        }
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.requests
            .values()
            .copied()
            .chain(self.lifecycle_deadline)
            .min()
    }

    pub fn tick(&mut self, now_milliseconds: u64) {
        if let Some((&request_id, _)) = self
            .requests
            .iter()
            .find(|(_, deadline)| now_milliseconds >= **deadline)
        {
            self.fail(SessionFailure::RequestTimedOut(request_id));
            return;
        }
        if self
            .lifecycle_deadline
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

    fn fail(&mut self, failure: SessionFailure) {
        self.state = SessionState::Failed(failure);
        self.lifecycle_deadline = None;
        self.requests.clear();
    }
}

fn decode_failure(error: DecodeError) -> SessionFailure {
    match error {
        DecodeError::MalformedPayload
        | DecodeError::TruncatedFrame { .. }
        | DecodeError::LimitExceeded(_) => SessionFailure::MalformedMessage,
    }
}

fn superseded_request(queue: &VecDeque<Message>, incoming: &Message) -> Option<u64> {
    match (queue.back(), incoming) {
        (Some(Message::Screens(current)), Message::Screens(_)) => Some(current.request_id),
        _ => None,
    }
}

fn coalesce(queue: &mut VecDeque<Message>, incoming: &Message) -> bool {
    let Some(last) = queue.back_mut() else {
        return false;
    };
    match (last, incoming) {
        (Message::Screens(current), Message::Screens(incoming)) => {
            *current = incoming.clone();
            true
        }
        (Message::Input(current), Message::Input(incoming))
            if current.screen == incoming.screen =>
        {
            current.merge(incoming);
            true
        }
        _ => false,
    }
}

impl InputBatch {
    pub fn new(screen: crate::ScreenId, every_sample: bool) -> Self {
        Self {
            screen,
            events: Vec::new(),
            every_sample,
        }
    }

    pub fn merge(&mut self, incoming: &InputBatch) {
        self.every_sample |= incoming.every_sample;
        for event in &incoming.events {
            self.push(event.clone());
        }
    }

    pub fn push(&mut self, event: InputEvent) {
        if let Some(last) = self.events.last_mut()
            && coalesce_input(last, &event)
        {
            return;
        }
        if !self.every_sample
            && let Some(mover) = mover(&event)
        {
            self.drop_move_of(mover);
        }
        self.events.push(event);
    }

    fn drop_move_of(&mut self, pointer: Mover) {
        for index in (0..self.events.len()).rev() {
            match &self.events[index] {
                InputEvent::Predicted { .. } => {}
                event => match mover(event) {
                    Some(held) if held == pointer => {
                        self.events.remove(index);
                        if matches!(self.events.get(index), Some(InputEvent::Predicted { .. })) {
                            self.events.remove(index);
                        }
                        return;
                    }
                    Some(_) => {}
                    None => return,
                },
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mover {
    Mouse,
    Finger(u64, u64),
    Pen,
}

fn mover(event: &InputEvent) -> Option<Mover> {
    match event {
        InputEvent::PointerMoved { .. } => Some(Mover::Mouse),
        InputEvent::Touch {
            device,
            finger,
            phase: crate::TouchPhase::Move,
            ..
        } => Some(Mover::Finger(*device, *finger)),
        InputEvent::Pen {
            phase: crate::PenPhase::Move,
            ..
        } => Some(Mover::Pen),
        _ => None,
    }
}

fn coalesce_input(current: &mut InputEvent, incoming: &InputEvent) -> bool {
    match (current, incoming) {
        (
            InputEvent::Wheel { x, y, unit },
            InputEvent::Wheel {
                x: new_x,
                y: new_y,
                unit: new_unit,
            },
        ) if unit == new_unit => {
            *x += new_x;
            *y += new_y;
            true
        }
        (InputEvent::Zoom { factor }, InputEvent::Zoom { factor: incoming }) => {
            *factor *= incoming;
            true
        }
        (InputEvent::Modifiers(current), InputEvent::Modifiers(incoming))
            if current == incoming =>
        {
            true
        }
        _ => false,
    }
}

impl From<InputBatch> for Message {
    fn from(input: InputBatch) -> Self {
        Self::Input(input)
    }
}

#[cfg(test)]
mod tests;
