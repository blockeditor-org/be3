use crate::Size;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Write(usize),
    Wait(Vec<(usize, u64)>),
    Busy,
}

#[derive(Clone, Debug, Default)]
struct Slot {
    size: Option<Size>,
    holders: u32,
    release: u64,
    owed: bool,
}

#[derive(Clone, Debug)]
pub struct Ring {
    slots: Vec<Slot>,
    latest: Option<usize>,
    writing: Option<usize>,
}

impl Ring {
    pub fn new(count: usize) -> Ring {
        Ring {
            slots: vec![Slot::default(); count],
            latest: None,
            writing: None,
        }
    }

    pub fn latest(&self) -> Option<usize> {
        self.latest
    }

    pub fn size(&self, slot: usize) -> Option<Size> {
        self.slots[slot].size
    }

    pub fn release_value(&self, slot: usize) -> u64 {
        self.slots[slot].release
    }

    pub fn hold_latest(&mut self) -> Option<usize> {
        let slot = self.latest?;
        self.slots[slot].holders += 1;
        Some(slot)
    }

    pub fn release(&mut self, slot: usize, completed: u64) -> Option<u64> {
        let state = &mut self.slots[slot];
        state.holders = state.holders.checked_sub(1)?;
        if state.holders > 0 {
            return None;
        }
        if state.owed || completed < state.release {
            state.owed = true;
            return None;
        }
        state.release += 1;
        Some(state.release)
    }

    pub fn settle(&mut self, completed: &[u64]) -> Vec<(usize, u64)> {
        let mut signals = Vec::new();
        for (index, state) in self.slots.iter_mut().enumerate() {
            if state.owed && completed[index] >= state.release {
                state.owed = false;
                state.release += 1;
                signals.push((index, state.release));
            }
        }
        signals
    }

    pub fn pick(&mut self, size: Size, completed: &[u64]) -> Pick {
        assert!(self.writing.is_none(), "a slot is already being written");
        let free = |index: usize, state: &Slot| Some(index) != self.latest && state.holders == 0;
        let ready = |index: usize, state: &Slot| {
            free(index, state) && !state.owed && completed[index] >= state.release
        };
        let candidates = || {
            self.slots
                .iter()
                .enumerate()
                .filter(|&(index, state)| ready(index, state))
        };
        let chosen = candidates()
            .find(|(_, state)| state.size == Some(size))
            .or_else(|| candidates().find(|(_, state)| state.size.is_none()))
            .or_else(|| candidates().next())
            .map(|(index, _)| index);
        if let Some(index) = chosen {
            self.writing = Some(index);
            return Pick::Write(index);
        }
        let waits: Vec<(usize, u64)> = self
            .slots
            .iter()
            .enumerate()
            .filter(|&(index, state)| free(index, state))
            .map(|(index, state)| (index, state.release))
            .collect();
        match waits.is_empty() {
            true => Pick::Busy,
            false => Pick::Wait(waits),
        }
    }

    pub fn written(&mut self, slot: usize, size: Size) {
        assert_eq!(self.writing, Some(slot), "the slot written was not picked");
        self.writing = None;
        self.slots[slot].size = Some(size);
        self.latest = Some(slot);
    }

    pub fn abandon(&mut self, slot: usize) {
        if self.writing == Some(slot) {
            self.writing = None;
        }
    }
}
