use block_plugin_api::RemapState;

#[derive(Default)]
pub(super) struct Approval {
    approved: Option<Vec<u8>>,
    wanted: Option<Vec<u8>>,
    declined: Option<Vec<u8>>,
    asking: bool,
}

impl Approval {
    pub(super) fn load(&mut self, approved: Option<Vec<u8>>) {
        self.approved = approved;
        self.asking = self.waiting() && self.wanted != self.declined;
    }

    pub(super) fn want(&mut self, wanted: Option<Vec<u8>>) -> bool {
        if self.wanted == wanted {
            return false;
        }
        self.wanted = wanted;
        self.asking = self.waiting() && self.wanted != self.declined;
        true
    }

    pub(super) fn review(&mut self) {
        self.asking = self.waiting();
    }

    pub(super) fn turn_on(&mut self) -> Option<Vec<u8>> {
        self.asking = false;
        self.approved.clone_from(&self.wanted);
        self.approved.clone()
    }

    pub(super) fn keep_off(&mut self) {
        self.asking = false;
        self.declined.clone_from(&self.wanted);
    }

    pub(super) fn asking(&self) -> bool {
        self.asking
    }

    pub(super) fn running(&self) -> Option<Vec<u8>> {
        match &self.wanted {
            None => None,
            Some(wanted) if self.approved.as_ref() == Some(wanted) => Some(wanted.clone()),
            Some(_) => self.approved.clone(),
        }
    }

    pub(super) fn state(&self, stopped: bool) -> RemapState {
        if self.wanted.is_none() {
            RemapState::Off
        } else if self.waiting() {
            RemapState::Waiting
        } else if stopped {
            RemapState::Stopped
        } else {
            RemapState::On
        }
    }

    fn waiting(&self) -> bool {
        self.wanted.is_some() && self.wanted != self.approved
    }
}

#[cfg(test)]
mod tests;
