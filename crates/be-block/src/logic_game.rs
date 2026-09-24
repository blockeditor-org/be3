use std::collections::{BTreeMap, HashSet};

use be_model::{Anchor, Change, Document, Edit, List, Map, Model, ObjectId};
use logicgame::challenges::{CHALLENGES, ChallengeId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChildChange, Root};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub challenge: ChallengeId,
    pub solutions: Vec<Uuid>,
    pub completed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuizProblem {
    pub carries: Vec<Option<bool>>,
    pub sums: Vec<Option<bool>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum QuizRow {
    Carries,
    Sums,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogicGame {
    levels: Vec<Level>,
    quiz: BTreeMap<usize, QuizProblem>,
}

impl LogicGame {
    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    pub fn level(&self, challenge: ChallengeId) -> Option<&Level> {
        self.levels
            .iter()
            .find(|level| level.challenge == challenge)
    }

    pub fn quiz(&self, problem: usize) -> Option<&QuizProblem> {
        self.quiz.get(&problem)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogicGameOperation {
    InsertSolution {
        challenge: ChallengeId,
        solution: Uuid,
        index: usize,
    },
    RemoveSolution {
        challenge: ChallengeId,
        solution: Uuid,
    },
    SetCompleted {
        challenge: ChallengeId,
        completed: bool,
    },
    SetQuizRow {
        problem: usize,
        row: QuizRow,
        values: Vec<Option<bool>>,
    },
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct LogicGameProgress {
    pub solutions: List<Solution>,
    pub completed: Map<ChallengeId, bool>,
    pub quiz: Map<(u32, QuizRow), Vec<Option<bool>>>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Solution {
    pub challenge: Option<ChallengeId>,
    pub block: Option<Uuid>,
}

impl LogicGameProgress {
    pub fn game(&self) -> LogicGame {
        let levels = CHALLENGES
            .into_iter()
            .map(|challenge| Level {
                challenge,
                solutions: self
                    .solutions_of(challenge)
                    .map(|(_, block)| block)
                    .collect(),
                completed: self.completed.get(&challenge).copied().unwrap_or(false),
            })
            .collect();
        let mut quiz: BTreeMap<usize, QuizProblem> = BTreeMap::new();
        for ((problem, row), values) in self.quiz.iter() {
            let answers = quiz.entry(*problem as usize).or_default();
            match row {
                QuizRow::Carries => answers.carries.clone_from(values),
                QuizRow::Sums => answers.sums.clone_from(values),
            }
        }
        LogicGame { levels, quiz }
    }

    fn solutions_of(&self, challenge: ChallengeId) -> impl Iterator<Item = (ObjectId, Uuid)> + '_ {
        self.solutions.iter().filter_map(move |solution| {
            (solution.challenge == Some(challenge)).then_some((solution.id, solution.block?))
        })
    }

    fn anchor_before(&self, id: ObjectId) -> Anchor {
        let position = self
            .solutions
            .iter()
            .position(|held| held.id == id)
            .unwrap_or(0);
        match position.checked_sub(1) {
            Some(previous) => Anchor::After(self.solutions[previous].id),
            None => Anchor::Start,
        }
    }

    pub fn edit_for(&self, operation: &LogicGameOperation) -> Edit {
        match operation {
            LogicGameOperation::InsertSolution {
                challenge,
                solution,
                index,
            } => {
                let existing: Vec<(ObjectId, Uuid)> = self.solutions_of(*challenge).collect();
                if existing.iter().any(|(_, block)| block == solution) {
                    return Edit::default();
                }
                let anchor = match (existing.first(), *index) {
                    (None, _) => Anchor::End,
                    (Some((first, _)), 0) => self.anchor_before(*first),
                    (Some(_), index) => Anchor::After(existing[index.min(existing.len()) - 1].0),
                };
                Self::SOLUTIONS
                    .insert(
                        ObjectId::ROOT,
                        anchor,
                        &Solution {
                            challenge: Some(*challenge),
                            block: Some(*solution),
                        },
                    )
                    .1
                    .into()
            }
            LogicGameOperation::RemoveSolution {
                challenge,
                solution,
            } => self
                .solutions_of(*challenge)
                .filter(|(_, block)| block == solution)
                .map(|(id, _)| Change::remove(id))
                .collect(),
            LogicGameOperation::SetCompleted {
                challenge,
                completed,
            } => Self::COMPLETED
                .put(ObjectId::ROOT, challenge, Some(completed))
                .into(),
            LogicGameOperation::SetQuizRow {
                problem,
                row,
                values,
            } => {
                let problem = u32::try_from(*problem).unwrap_or(u32::MAX);
                Self::QUIZ
                    .put(ObjectId::ROOT, &(problem, *row), Some(values))
                    .into()
            }
        }
    }
}

impl Root for LogicGameProgress {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6c6f_6769_632d_6761_6d65_2d63_6f6e_0002);

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.solutions
            .iter()
            .filter_map(|solution| solution.block)
            .filter(|block| seen.insert(*block))
            .collect()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) => None,
            ChildChange::Delete(old) => Some(
                self.solutions
                    .iter()
                    .filter(|solution| solution.block == Some(old))
                    .map(|solution| Change::remove(solution.id))
                    .collect(),
            ),
            ChildChange::Replace { old, new } => Some(
                self.solutions
                    .iter()
                    .filter(|solution| solution.block == Some(old))
                    .map(|solution| Solution::BLOCK.set(solution.id, &Some(new)))
                    .collect(),
            ),
        }
    }
}

pub type LogicGameContent = Document<LogicGameProgress>;
