use block_plugin_api::HostWindowId;

#[derive(Default)]
pub(crate) struct Shown {
    hidden: Vec<HostWindowId>,
    changes: u64,
}

impl Shown {
    pub(crate) fn set(&mut self, window: HostWindowId, shown: bool) -> bool {
        let was = !self.hidden.contains(&window);
        if was == shown {
            return false;
        }
        match shown {
            true => self.hidden.retain(|hidden| *hidden != window),
            false => self.hidden.push(window),
        }
        self.changes += 1;
        true
    }

    pub(crate) fn shown(&self, window: HostWindowId) -> bool {
        !self.hidden.contains(&window)
    }

    pub(crate) fn keep_only(&mut self, listed: &[HostWindowId]) {
        self.hidden.retain(|hidden| listed.contains(hidden));
    }

    pub(crate) fn changes(&self) -> u64 {
        self.changes
    }
}

#[cfg(test)]
mod tests;
