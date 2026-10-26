pub(crate) const GRACE_USEC: u64 = 750_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Held {
    Key(u32),
    Button(u32),
    Touch(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Input {
    Press(Held),
    Continue(Held),
    Release(Held),
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Pass {
    pub(crate) deliver: bool,
    pub(crate) woke: bool,
}

#[derive(Default)]
pub(crate) struct WakeGate {
    blanked: bool,
    grace_until: Option<u64>,
    swallowed: Vec<Held>,
}

impl WakeGate {
    pub(crate) fn set_blanked(&mut self, blanked: bool) {
        self.blanked = blanked;
        if blanked {
            self.grace_until = None;
        }
    }

    pub(crate) fn pass(&mut self, input: Input, time_usec: u64) -> Pass {
        let woke = std::mem::take(&mut self.blanked);
        if woke {
            self.grace_until = Some(time_usec.saturating_add(GRACE_USEC));
        }
        let guarded = woke || self.grace_until.is_some_and(|until| time_usec < until);
        let deliver = match input {
            Input::Release(held) => match self.swallowed.iter().position(|known| *known == held) {
                Some(index) => {
                    self.swallowed.remove(index);
                    false
                }
                None => true,
            },
            Input::Continue(held) => !guarded && !self.swallowed.contains(&held),
            Input::Press(held) if guarded => {
                if !self.swallowed.contains(&held) {
                    self.swallowed.push(held);
                }
                false
            }
            Input::Press(_) | Input::Other => !guarded,
        };
        Pass { deliver, woke }
    }
}

#[cfg(test)]
mod tests;
