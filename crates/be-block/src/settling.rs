use std::collections::VecDeque;

use crate::{LiveEdit, Touched};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Landed<Op> {
    On {
        operation: Op,
        unsettled: u64,
    },
    Beneath {
        operation: Op,
        unsettled: u64,
        local: Vec<Op>,
    },
}

impl<Op> Landed<Op> {
    pub fn map<T>(&self, mut map: impl FnMut(&Op) -> T) -> Landed<T> {
        match self {
            Self::On {
                operation,
                unsettled,
            } => Landed::On {
                operation: map(operation),
                unsettled: *unsettled,
            },
            Self::Beneath {
                operation,
                unsettled,
                local,
            } => Landed::Beneath {
                operation: map(operation),
                unsettled: *unsettled,
                local: local.iter().map(map).collect(),
            },
        }
    }
}

#[derive(Clone)]
pub struct Settling<C: LiveEdit> {
    settled: C,
    unsettled: VecDeque<C::Op>,
}

impl<C: LiveEdit> Settling<C> {
    pub fn new(settled: C, unsettled: Vec<C::Op>) -> Self {
        Self {
            settled,
            unsettled: unsettled.into(),
        }
    }

    pub fn settled(&self) -> &C {
        &self.settled
    }

    pub fn unsettled(&self) -> &VecDeque<C::Op> {
        &self.unsettled
    }

    pub fn settle_to(&mut self, unsettled: u64) {
        while self.unsettled.len() as u64 > unsettled {
            if let Some(operation) = self.unsettled.pop_front() {
                self.settled.apply(&operation);
            }
        }
    }

    pub fn push(&mut self, operation: C::Op, unsettled: u64) {
        self.unsettled.push_back(operation);
        self.settle_to(unsettled);
    }

    pub fn beneath(
        &mut self,
        visible: &mut C,
        operation: &C::Op,
        unsettled: u64,
        local: Vec<C::Op>,
        above: impl IntoIterator<Item = C::Op>,
        touched: &mut Vec<Touched>,
    ) -> bool
    where
        C: Clone,
    {
        self.settle_to(unsettled);
        if (self.unsettled.len() as u64) < unsettled {
            return false;
        }
        let ahead: Vec<C::Op> = local.iter().cloned().chain(above).collect();
        visible.apply_beneath(&mut self.settled, operation, &ahead, touched);
        self.unsettled = local.into();
        true
    }

    pub fn land(&mut self, landed: Landed<C::Op>) -> bool {
        match landed {
            Landed::On {
                operation,
                unsettled,
            } => {
                self.push(operation, unsettled);
                true
            }
            Landed::Beneath {
                operation,
                unsettled,
                local,
            } => {
                self.settle_to(unsettled);
                if (self.unsettled.len() as u64) < unsettled {
                    return false;
                }
                self.settled.apply(&operation);
                self.unsettled = local.into();
                true
            }
        }
    }

    pub fn visible(&self) -> C
    where
        C: Clone,
    {
        let mut visible = self.settled.clone();
        for operation in &self.unsettled {
            visible.apply(operation);
        }
        visible
    }
}
