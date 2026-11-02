use super::{Input, PenPhase, ScrollDelta, TouchId, TouchPhase};

pub trait Queued {
    fn input(&self) -> Option<&Input>;

    fn input_mut(&mut self) -> Option<&mut Input>;

    fn joins(&self, _later: &Self) -> bool {
        true
    }
}

pub fn enqueue<T: Queued>(queue: &mut Vec<T>, entry: T, every_sample: bool) {
    if let Some(last) = queue.last_mut()
        && last.joins(&entry)
        && let (Some(current), Some(incoming)) = (last.input_mut(), entry.input())
        && coalesce(current, incoming)
    {
        return;
    }
    if !every_sample && let Some(pointer) = entry.input().and_then(mover) {
        drop_move_of(queue, pointer);
    }
    queue.push(entry);
}

fn drop_move_of<T: Queued>(queue: &mut Vec<T>, pointer: Mover) {
    let predicted =
        |entry: Option<&T>| matches!(entry.and_then(Queued::input), Some(Input::Predicted(_)));
    for index in (0..queue.len()).rev() {
        if predicted(queue.get(index)) {
            continue;
        }
        match queue[index].input().and_then(mover) {
            Some(held) if held == pointer => {
                queue.remove(index);
                if predicted(queue.get(index)) {
                    queue.remove(index);
                }
                return;
            }
            Some(_) => {}
            None => return,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mover {
    Mouse,
    Finger(TouchId),
    Pen,
}

fn mover(input: &Input) -> Option<Mover> {
    match input {
        Input::PointerMoved(_) => Some(Mover::Mouse),
        Input::Touch {
            id,
            phase: TouchPhase::Move,
            ..
        } => Some(Mover::Finger(*id)),
        Input::Pen {
            phase: PenPhase::Move,
            ..
        } => Some(Mover::Pen),
        _ => None,
    }
}

fn coalesce(current: &mut Input, incoming: &Input) -> bool {
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
