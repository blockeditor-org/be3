#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Streak {
    Rebuilt,
    Stalled,
}

pub(crate) trait Card {
    type Output: Copy + Eq;

    fn suspend(&mut self);
    fn release(&mut self, output: Option<Self::Output>);
    fn take_back(&mut self);
    fn rescan(&mut self);
    fn stall(&mut self, output: Self::Output);
    fn report(&mut self, problem: String);
}

pub(crate) struct Recovery<K> {
    active: bool,
    streaks: Vec<(K, Streak)>,
}

impl<K> Default for Recovery<K> {
    fn default() -> Self {
        Self {
            active: true,
            streaks: Vec::new(),
        }
    }
}

impl<K: Copy + Eq> Recovery<K> {
    pub(crate) fn active(&self) -> bool {
        self.active
    }

    pub(crate) fn pause(&mut self, card: &mut impl Card<Output = K>) {
        if !self.active {
            return;
        }
        self.active = false;
        card.suspend();
    }

    pub(crate) fn activate(&mut self, card: &mut impl Card<Output = K>) {
        self.active = true;
        self.streaks.clear();
        card.release(None);
        card.take_back();
        card.rescan();
    }

    pub(crate) fn changed(&mut self, card: &mut impl Card<Output = K>) {
        if !self.active {
            return;
        }
        for (output, streak) in std::mem::take(&mut self.streaks) {
            if streak == Streak::Stalled {
                card.release(Some(output));
            }
        }
        card.rescan();
    }

    pub(crate) fn shown(&mut self, output: K) {
        self.streaks.retain(|(known, _)| *known != output);
    }

    pub(crate) fn failed(&mut self, card: &mut impl Card<Output = K>, output: K, problem: String) {
        if !self.active {
            return;
        }
        match self.streaks.iter_mut().find(|(known, _)| *known == output) {
            None => {
                eprintln!("beui: {problem}, so the display is set up again");
                self.streaks.push((output, Streak::Rebuilt));
                card.release(Some(output));
                card.rescan();
            }
            Some((_, streak @ Streak::Rebuilt)) => {
                *streak = Streak::Stalled;
                card.stall(output);
                card.report(format!(
                    "{problem}, even after setting it up again, so it stays dark until you switch \
                     back to this terminal or reconnect it."
                ));
            }
            Some((_, Streak::Stalled)) => card.stall(output),
        }
    }
}

#[cfg(test)]
mod tests;
