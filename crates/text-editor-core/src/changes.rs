use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextChange {
    pub start: usize,
    pub old_end: usize,
    pub new_end: usize,
}

impl TextChange {
    pub const NONE: Self = Self {
        start: 0,
        old_end: 0,
        new_end: 0,
    };

    pub const fn replace(index: usize, delete: usize, insert: usize) -> Self {
        Self {
            start: index,
            old_end: index + delete,
            new_end: index + insert,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.old_end && self.start == self.new_end
    }

    pub fn then(self, next: Self) -> Self {
        if self.is_empty() {
            return next;
        }
        if next.is_empty() {
            return self;
        }
        let start = self.start.min(next.start);
        let middle = self.new_end.max(next.old_end);
        Self {
            start,
            old_end: middle - self.new_end + self.old_end,
            new_end: middle - next.old_end + next.new_end,
        }
    }

    pub fn moved(self, index: usize) -> Option<usize> {
        if index <= self.start {
            Some(index)
        } else if index >= self.old_end {
            Some(index - self.old_end + self.new_end)
        } else {
            None
        }
    }
}

const CHANGES_KEPT: usize = 256;

#[derive(Default)]
pub struct ChangeLog {
    entries: VecDeque<(u64, Option<TextChange>)>,
}

impl ChangeLog {
    pub fn record(&mut self, revision: u64, change: Option<TextChange>) {
        if self.entries.len() >= CHANGES_KEPT {
            self.entries.pop_front();
        }
        self.entries.push_back((revision, change));
    }

    pub fn since(&self, from: u64, to: u64) -> Option<TextChange> {
        if from == to {
            return Some(TextChange::NONE);
        }
        let first = self.entries.iter().position(|(revision, _)| *revision == from + 1)?;
        let mut expected = from + 1;
        let mut total = TextChange::NONE;
        for (revision, change) in self.entries.iter().skip(first) {
            if *revision != expected || *revision > to {
                return None;
            }
            total = total.then((*change)?);
            if *revision == to {
                return Some(total);
            }
            expected += 1;
        }
        None
    }
}
