use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::names::{Modifier, Source};
use crate::rules::{Action, Chord, Group, Held, Rule, Step, Trigger, Wheel, Window};

pub const WHEEL_NOTCH: i32 = 120;
const MICROS_PER_MILLI: u64 = 1000;
const MOST_SCHEDULED: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Input {
    Press(Source),
    Release(Source),
    Wheel { x: i32, y: i32 },
}

pub type Output = Input;

#[derive(Clone, Debug)]
enum Taken {
    Mirror { chord: Chord, keep: bool },
    Swallowed { keep: bool },
}

#[derive(Clone, Debug)]
enum Scheduled {
    Emit(Output),
    Lift(BTreeSet<Source>),
    Restore,
}

pub struct Engine {
    groups: Vec<Group>,
    window: Option<Window>,
    physical: BTreeSet<Source>,
    downstream: BTreeSet<Source>,
    taken: BTreeMap<Source, Taken>,
    schedule: VecDeque<(u64, Scheduled)>,
    busy_until: u64,
    lifted: Vec<Source>,
    wheel: [i32; 2],
}

impl Engine {
    pub fn new(
        groups: Vec<Group>,
        window: Option<Window>,
        held: &[Source],
        pressed: &[Source],
    ) -> Self {
        let mut ordered: Vec<Group> = groups
            .iter()
            .filter(|group| group.window.is_some())
            .cloned()
            .collect();
        ordered.extend(groups.into_iter().filter(|group| group.window.is_none()));
        Self {
            groups: ordered,
            window,
            physical: held.iter().copied().collect(),
            downstream: pressed.iter().copied().collect(),
            taken: BTreeMap::new(),
            schedule: VecDeque::new(),
            busy_until: 0,
            lifted: Vec::new(),
            wheel: [0; 2],
        }
    }

    pub fn focus(&mut self, window: Option<Window>) {
        self.window = window;
    }

    pub fn deadline(&self) -> Option<u64> {
        self.schedule.front().map(|(at, _)| *at)
    }

    pub fn timer(&mut self, now: u64) -> Vec<Output> {
        let mut out = Vec::new();
        self.due(now, &mut out);
        out
    }

    pub fn input(&mut self, input: Input, now: u64) -> Vec<Output> {
        let mut out = Vec::new();
        self.due(now, &mut out);
        match input {
            Input::Press(source) => self.press(source, now, &mut out),
            Input::Release(source) => self.release(source, &mut out),
            Input::Wheel { x, y } => self.turn(x, y, now, &mut out),
        }
        self.due(now, &mut out);
        out
    }

    fn press(&mut self, source: Source, now: u64, out: &mut Vec<Output>) {
        if self.taken.contains_key(&source) {
            return;
        }
        self.physical.insert(source);
        let Some(rule) = self.rule(Trigger::Press(source)) else {
            self.emit(Input::Press(source), out);
            return;
        };
        if rule.keep {
            self.emit(Input::Press(source), out);
        }
        let taken = match rule.action {
            Action::Send(chord) => {
                for source in chord.sources() {
                    self.emit(Input::Press(source), out);
                }
                Taken::Mirror {
                    chord,
                    keep: rule.keep,
                }
            }
            Action::Steps { steps, blind } => {
                self.play(&steps, blind, now);
                Taken::Swallowed { keep: rule.keep }
            }
        };
        self.taken.insert(source, taken);
    }

    fn release(&mut self, source: Source, out: &mut Vec<Output>) {
        self.physical.remove(&source);
        match self.taken.remove(&source) {
            None => self.emit(Input::Release(source), out),
            Some(Taken::Mirror { chord, keep }) => {
                let sources: Vec<Source> = chord.sources().collect();
                for held in sources.into_iter().rev() {
                    self.let_go(held, out);
                }
                if keep {
                    self.emit(Input::Release(source), out);
                }
            }
            Some(Taken::Swallowed { keep }) => {
                if keep {
                    self.emit(Input::Release(source), out);
                }
            }
        }
    }

    fn turn(&mut self, x: i32, y: i32, now: u64, out: &mut Vec<Output>) {
        let mut passed = [0; 2];
        for (axis, value, back, forth) in [
            (0, x, Wheel::Left, Wheel::Right),
            (1, y, Wheel::Up, Wheel::Down),
        ] {
            if value == 0 {
                continue;
            }
            let direction = match value < 0 {
                true => back,
                false => forth,
            };
            let Some(rule) = self.rule(Trigger::Wheel(direction)) else {
                passed[axis] = value;
                self.wheel[axis] = 0;
                continue;
            };
            if rule.keep {
                passed[axis] = value;
            }
            if self.wheel[axis].signum() != value.signum() {
                self.wheel[axis] = 0;
            }
            self.wheel[axis] = self.wheel[axis].saturating_add(value);
            while self.wheel[axis].abs() >= WHEEL_NOTCH {
                self.wheel[axis] -= WHEEL_NOTCH * value.signum();
                match &rule.action {
                    Action::Send(chord) => {
                        let tap = [Step::Tap {
                            chord: chord.clone(),
                            hold_ms: 0,
                        }];
                        self.play(&tap, true, now);
                    }
                    Action::Steps { steps, blind } => self.play(steps, *blind, now),
                }
            }
        }
        if passed != [0, 0] {
            out.push(Input::Wheel {
                x: passed[0],
                y: passed[1],
            });
        }
    }

    fn rule(&self, trigger: Trigger) -> Option<Rule> {
        let applies = |group: &&Group| match (&group.window, &self.window) {
            (None, _) => true,
            (Some(pattern), Some(window)) => pattern.matches(window),
            (Some(_), None) => false,
        };
        self.groups
            .iter()
            .filter(applies)
            .flat_map(|group| &group.rules)
            .find(|rule| {
                rule.on == trigger
                    && rule.held.iter().all(|held| self.is_held(*held))
                    && !rule.not_held.iter().any(|held| self.is_held(*held))
            })
            .cloned()
    }

    fn is_held(&self, held: Held) -> bool {
        match held {
            Held::Source(source) => self.physical.contains(&source),
            Held::Modifier(modifier) => modifier
                .keys()
                .iter()
                .any(|source| self.physical.contains(source)),
        }
    }

    fn play(&mut self, steps: &[Step], blind: bool, now: u64) {
        if self.schedule.len() >= MOST_SCHEDULED {
            return;
        }
        let mut at = now.max(self.busy_until);
        if !blind {
            let named = steps
                .iter()
                .filter_map(|step| match step {
                    Step::Tap { chord, .. } => Some(chord.sources()),
                    Step::Wait { .. } => None,
                })
                .flatten()
                .collect();
            self.schedule.push_back((at, Scheduled::Lift(named)));
        }
        for step in steps {
            match step {
                Step::Tap { chord, hold_ms } => {
                    for source in chord.sources() {
                        self.schedule
                            .push_back((at, Scheduled::Emit(Input::Press(source))));
                    }
                    at = at.saturating_add(u64::from(*hold_ms) * MICROS_PER_MILLI);
                    let sources: Vec<Source> = chord.sources().collect();
                    for source in sources.into_iter().rev() {
                        self.schedule
                            .push_back((at, Scheduled::Emit(Input::Release(source))));
                    }
                }
                Step::Wait { ms } => at = at.saturating_add(u64::from(*ms) * MICROS_PER_MILLI),
            }
        }
        if !blind {
            self.schedule.push_back((at, Scheduled::Restore));
        }
        self.busy_until = at;
    }

    fn due(&mut self, now: u64, out: &mut Vec<Output>) {
        while self.schedule.front().is_some_and(|(at, _)| *at <= now) {
            let Some((_, scheduled)) = self.schedule.pop_front() else {
                break;
            };
            match scheduled {
                Scheduled::Emit(Input::Release(source)) => self.let_go(source, out),
                Scheduled::Emit(output) => self.emit(output, out),
                Scheduled::Lift(named) => {
                    let lifted: Vec<Source> = Modifier::ALL
                        .iter()
                        .flat_map(|modifier| modifier.keys())
                        .filter(|source| {
                            self.downstream.contains(source) && !named.contains(source)
                        })
                        .collect();
                    for source in &lifted {
                        self.emit(Input::Release(*source), out);
                    }
                    self.lifted = lifted;
                }
                Scheduled::Restore => {
                    for source in std::mem::take(&mut self.lifted) {
                        if self.passing(source) {
                            self.emit(Input::Press(source), out);
                        }
                    }
                }
            }
        }
    }

    fn passing(&self, source: Source) -> bool {
        self.physical.contains(&source) && !self.taken.contains_key(&source)
    }

    fn let_go(&mut self, source: Source, out: &mut Vec<Output>) {
        if !self.passing(source) {
            self.emit(Input::Release(source), out);
        }
    }

    fn emit(&mut self, output: Output, out: &mut Vec<Output>) {
        let changed = match output {
            Input::Press(source) => self.downstream.insert(source),
            Input::Release(source) => self.downstream.remove(&source),
            Input::Wheel { .. } => true,
        };
        if changed {
            out.push(output);
        }
    }
}
