use crate::{Input, InputBatch, ScrollDelta, TimedInput};

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
        let event = event.into();
        if let Some(last) = self.events.last_mut()
            && last.layer == event.layer
            && coalesce_input(&mut last.input, &event.input)
        {
            return;
        }
        if !self.every_sample
            && let Some(mover) = mover(&event.input)
        {
            self.drop_move_of(mover);
        }
        self.events.push(event);
    }

    fn drop_move_of(&mut self, pointer: Mover) {
        let predicted = |event: Option<&TimedInput>| {
            matches!(
                event,
                Some(TimedInput {
                    input: Input::Predicted(_),
                    ..
                })
            )
        };
        for index in (0..self.events.len()).rev() {
            if predicted(self.events.get(index)) {
                continue;
            }
            match mover(&self.events[index].input) {
                Some(held) if held == pointer => {
                    self.events.remove(index);
                    if predicted(self.events.get(index)) {
                        self.events.remove(index);
                    }
                    return;
                }
                Some(_) => {}
                None => return,
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mover {
    Mouse,
    Finger(crate::TouchId),
    Pen,
}

fn mover(input: &Input) -> Option<Mover> {
    match input {
        Input::PointerMoved(_) => Some(Mover::Mouse),
        Input::Touch {
            id,
            phase: crate::TouchPhase::Move,
            ..
        } => Some(Mover::Finger(*id)),
        Input::Pen {
            phase: crate::PenPhase::Move,
            ..
        } => Some(Mover::Pen),
        _ => None,
    }
}

fn coalesce_input(current: &mut Input, incoming: &Input) -> bool {
    match (current, incoming) {
        (Input::Scroll(current), Input::Scroll(incoming)) => match (current, incoming) {
            (ScrollDelta::Pixels(current), ScrollDelta::Pixels(incoming))
            | (ScrollDelta::Lines(current), ScrollDelta::Lines(incoming))
            | (ScrollDelta::Pages(current), ScrollDelta::Pages(incoming)) => {
                *current += *incoming;
                true
            }
            _ => false,
        },
        (Input::Zoom(factor), Input::Zoom(incoming)) => {
            *factor *= incoming;
            true
        }
        (Input::Modifiers(current), Input::Modifiers(incoming)) if current == incoming => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
