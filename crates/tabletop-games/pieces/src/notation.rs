use crate::checkers::dark;
use crate::{Position, Square, Step};

pub(crate) fn algebraic(position: &Position, step: &Step, legal: &[Step]) -> String {
    let Some(man) = position.at(step.from) else {
        return format!("{}{}", step.from, step.to);
    };
    if man.piece.royal()
        && let Some((rook, _)) = step.rider
    {
        return match rook.file > step.from.file {
            true => "O-O".to_owned(),
            false => "O-O-O".to_owned(),
        };
    }
    let mut written = man.piece.letter().to_owned();
    if !step.via.is_empty() {
        written.push_str(&step.from.to_string());
        for landing in step.via.iter().chain([&step.to]) {
            written.push('x');
            written.push_str(&landing.to_string());
        }
        return promoted(written, step);
    }
    let rivals: Vec<Square> = legal
        .iter()
        .filter(|other| other.to == step.to && other.from != step.from)
        .filter(|other| {
            position
                .at(other.from)
                .is_some_and(|rival| rival.piece.name() == man.piece.name())
        })
        .map(|other| other.from)
        .collect();
    let file = file_letter(step.from);
    let rank = (step.from.rank + 1).to_string();
    let lettered = !man.piece.letter().is_empty();
    if !lettered && step.is_capture() {
        written.push(file);
        if rivals.iter().any(|rival| rival.file == step.from.file) {
            written.push_str(&rank);
        }
    } else if !rivals.is_empty() {
        if rivals.iter().all(|rival| rival.file != step.from.file) {
            written.push(file);
        } else if rivals.iter().all(|rival| rival.rank != step.from.rank) {
            written.push_str(&rank);
        } else {
            written.push(file);
            written.push_str(&rank);
        }
    }
    if step.is_capture() {
        written.push('x');
    }
    written.push_str(&step.to.to_string());
    promoted(written, step)
}

fn promoted(mut written: String, step: &Step) -> String {
    if let Some(piece) = step.becomes
        && !piece.letter().is_empty()
    {
        written.push('=');
        written.push_str(piece.letter());
    }
    written
}

fn file_letter(square: Square) -> char {
    (b'a' + square.file as u8) as char
}

pub(crate) fn numbered(position: &Position, step: &Step) -> String {
    let number = |square: Square| number(position, square).to_string();
    if step.is_capture() {
        let mut written = number(step.from);
        for landing in step.via.iter().chain([&step.to]) {
            written.push('x');
            written.push_str(&number(*landing));
        }
        written
    } else {
        format!("{}-{}", number(step.from), number(step.to))
    }
}

pub fn number(position: &Position, square: Square) -> u32 {
    let per_rank = (0..position.columns())
        .filter(|file| dark(Square::new(*file, 0)))
        .count() as u32;
    let before = (square.file + 1..position.columns())
        .filter(|file| dark(Square::new(*file, square.rank)))
        .count() as u32;
    square.rank as u32 * per_rank + before + 1
}
