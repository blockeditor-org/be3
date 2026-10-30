use crate::{Input, InputBatch, TimedInput};

impl InputBatch {
    pub fn new(screen: crate::ScreenId, every_sample: bool) -> Self {
        Self {
            screen,
            events: Vec::new(),
            every_sample,
            dismissed: Vec::new(),
        }
    }

    pub fn queue(batches: &mut Vec<InputBatch>, incoming: InputBatch) {
        match batches.last_mut() {
            Some(last) if last.screen == incoming.screen && incoming.dismissed.is_empty() => {
                last.merge(&incoming)
            }
            _ => batches.push(incoming),
        }
    }

    pub fn merge(&mut self, incoming: &InputBatch) {
        self.every_sample |= incoming.every_sample;
        for event in &incoming.events {
            self.push(event.clone());
        }
    }

    pub fn push(&mut self, event: impl Into<TimedInput>) {
        ui_types::input::enqueue(&mut self.events, event.into(), self.every_sample);
    }
}

impl ui_types::input::Queued for TimedInput {
    fn input(&self) -> Option<&Input> {
        Some(&self.input)
    }

    fn input_mut(&mut self) -> Option<&mut Input> {
        Some(&mut self.input)
    }

    fn joins(&self, later: &Self) -> bool {
        self.over == later.over
    }
}

#[cfg(test)]
mod tests;
