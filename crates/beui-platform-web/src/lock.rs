use beui_types::{Input, PointerButton};

#[derive(Default)]
pub(crate) struct Lock {
    wanted: bool,
    held: bool,
    lost: bool,
    regrabbing: Vec<PointerButton>,
}

impl Lock {
    pub(crate) fn held(&self) -> bool {
        self.held
    }

    pub(crate) fn want(&mut self, wanted: bool) {
        self.wanted = wanted;
        self.lost = false;
    }

    pub(crate) fn changed(&mut self, held: bool) -> Option<Input> {
        let was = std::mem::replace(&mut self.held, held);
        if held {
            self.lost = false;
        }
        match was && !held {
            true => self.lose(),
            false => None,
        }
    }

    pub(crate) fn refused(&mut self) -> Option<Input> {
        match self.held {
            true => None,
            false => self.lose(),
        }
    }

    fn lose(&mut self) -> Option<Input> {
        if !self.wanted || self.lost {
            return None;
        }
        self.lost = true;
        Some(Input::PointerUnlocked)
    }

    pub(crate) fn press(&mut self, button: PointerButton) -> bool {
        if !self.lost || self.held || !self.wanted {
            return false;
        }
        self.lost = false;
        self.regrabbing.push(button);
        true
    }

    pub(crate) fn release(&mut self, button: PointerButton) -> bool {
        let Some(at) = self.regrabbing.iter().position(|held| *held == button) else {
            return false;
        };
        self.regrabbing.remove(at);
        true
    }
}

#[cfg(test)]
mod tests;
