use crate::app::ui::moves::{Cell, rows};
use crate::app::ui::{Table, Turn};

fn turn(description: &str, column: Option<u32>) -> Turn {
    Turn {
        description: description.to_owned(),
        player: String::new(),
        column,
    }
}

#[test]
fn consecutive_turns_of_one_player_share_a_cell() {
    let table = Table {
        columns: vec!["P1".to_owned(), "P2".to_owned()],
        history: vec![
            turn("deals", None),
            turn("draw", Some(0)),
            turn("7♥", Some(0)),
            turn("7♠", Some(1)),
            turn("8♠→♦", Some(0)),
        ],
        ending: None,
    };

    let rows = rows(&table);

    let cell = |text: &str, first, last| {
        Some(Cell {
            text: text.to_owned(),
            first,
            last,
        })
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].cells, [cell("draw 7♥", 1, 2), cell("7♠", 3, 3)]);
    assert_eq!(rows[1].cells, [cell("8♠→♦", 4, 4), None]);
}
