use std::time::Duration;

pub(crate) const FINGERS: usize = 4;
pub(crate) const HOLD: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(crate) struct Escape {
    touching: Vec<i32>,
}

impl Escape {
    pub(crate) fn touch(&mut self, finger: i32, down: bool) -> Option<bool> {
        let armed = self.armed();
        self.touching.retain(|known| *known != finger);
        if down {
            self.touching.push(finger);
        }
        (self.armed() != armed).then(|| self.armed())
    }

    fn armed(&self) -> bool {
        self.touching.len() >= FINGERS
    }
}

#[cfg(test)]
mod tests;
