use block_client::BlockHandle;
use block_client::blocks::logic_game::{LogicGame, LogicGameOperation, QuizRow};

pub(crate) mod ui;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BinaryAdditionProblem {
    operands: Vec<String>,
    carry_bits: Vec<bool>,
    sum_bits: Vec<bool>,
}

impl BinaryAdditionProblem {
    fn new(operands: &[u64]) -> Self {
        assert!(
            operands.len() >= 2,
            "binary addition problems need at least two operands"
        );
        let total = operands.iter().sum::<u64>();
        let operand_width = operands
            .iter()
            .map(|operand| binary_width(*operand))
            .max()
            .unwrap_or(1);
        let width = operand_width.max(binary_width(total));
        let mut carry_by_column = Vec::with_capacity(width.saturating_sub(1));
        let mut carry = 0u64;
        for column in 0..width {
            if column > 0 {
                carry_by_column.push(carry == 1);
            }
            let column_total = carry
                + operands
                    .iter()
                    .map(|operand| (operand >> column) & 1)
                    .sum::<u64>();
            carry = column_total >> 1;
        }
        carry_by_column.reverse();

        Self {
            operands: operands
                .iter()
                .map(|operand| format!("{operand:0width$b}"))
                .collect(),
            carry_bits: carry_by_column,
            sum_bits: (0..width)
                .rev()
                .map(|column| (total >> column) & 1 == 1)
                .collect(),
        }
    }

    pub(crate) fn operands(&self) -> &[String] {
        &self.operands
    }

    pub(crate) fn carry_bits(&self) -> &[bool] {
        &self.carry_bits
    }

    pub(crate) fn sum_bits(&self) -> &[bool] {
        &self.sum_bits
    }

    fn width(&self) -> usize {
        self.sum_bits.len()
    }
}

pub(crate) struct BinaryAdditionQuiz {
    problems: Vec<BinaryAdditionProblem>,
}

impl Default for BinaryAdditionQuiz {
    fn default() -> Self {
        Self {
            problems: vec![
                BinaryAdditionProblem::new(&[0b10101, 0b01011]),
                BinaryAdditionProblem::new(&[0b11010101, 0b00111110]),
                BinaryAdditionProblem::new(&[0b101101011011, 0b011011010101]),
            ],
        }
    }
}

impl BinaryAdditionQuiz {
    pub(crate) fn problems(&self) -> &[BinaryAdditionProblem] {
        &self.problems
    }

    pub(crate) fn fit(
        &self,
        problem: usize,
        carries: Vec<Option<bool>>,
        sums: Vec<Option<bool>>,
    ) -> (Vec<Option<bool>>, Vec<Option<bool>>) {
        (
            fitted(carries, self.problems[problem].carry_bits.len()),
            fitted(sums, self.problems[problem].width()),
        )
    }

    pub(crate) fn is_correct(
        &self,
        carries: &[Option<bool>],
        sums: &[Option<bool>],
        problem: usize,
    ) -> bool {
        let problem = &self.problems[problem];
        matches(carries, &problem.carry_bits) && matches(sums, &problem.sum_bits)
    }

    pub(crate) fn write_row(
        &self,
        block: &BlockHandle<LogicGame>,
        problem: usize,
        row: QuizRow,
        values: Vec<Option<bool>>,
    ) {
        block.operate(LogicGameOperation::SetQuizRow {
            problem,
            row,
            values,
        });
    }
}

pub(crate) fn next_answer(answer: Option<bool>) -> Option<bool> {
    match answer {
        None => Some(false),
        Some(false) => Some(true),
        Some(true) => None,
    }
}

fn matches(answers: &[Option<bool>], expected: &[bool]) -> bool {
    answers.len() == expected.len()
        && answers
            .iter()
            .zip(expected)
            .all(|(answer, expected)| *answer == Some(*expected))
}

fn fitted(mut values: Vec<Option<bool>>, length: usize) -> Vec<Option<bool>> {
    values.resize(length, None);
    values
}

fn binary_width(value: u64) -> usize {
    let width = u64::BITS - value.leading_zeros();
    width.max(1) as usize
}

#[cfg(test)]
mod tests;
