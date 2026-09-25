use crate::{Anchor, Change, Count, Document, Edit, List, Map, Model, ObjectId, Touched};

mod a_burst_of_sets_to_one_field_undoes_as_one_step;
mod a_card_edited_after_a_move_is_edited_where_it_went;
mod a_document_reads_back_what_it_was_built_from;
mod a_node_cannot_move_inside_itself;
mod an_edit_touches_its_field_and_what_holds_it;
mod counts_merge_by_adding_both_sides;
mod grids_merge_cell_by_cell_and_keep_coordinates_across_a_resize;
mod map_entries_merge_key_by_key;
mod merging_a_move_on_one_side_with_an_edit_on_the_other_keeps_both;
mod merging_restores_a_column_one_side_removed_while_the_other_filled_it;
mod one_field_set_on_both_sides_conflicts_and_keeps_ours;
mod undo_leaves_a_field_someone_else_changed_since;
mod undo_of_map_entries_restores_only_what_nobody_changed_since;
mod undo_puts_a_removed_column_back_with_its_cards;
mod undoing_paint_and_a_crop_restores_only_what_nobody_changed_since;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Board {
    title: String,
    votes: Count,
    columns: List<Column>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Column {
    name: String,
    cards: List<Card>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Card {
    text: String,
    done: bool,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Node {
    name: String,
    children: List<Node>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Sheet {
    cells: Map<u32, String>,
}

fn cell(document: &Document<Sheet>, key: u32) -> Option<String> {
    document.root().cells.get(&key).cloned()
}

fn card(text: &str) -> Card {
    Card {
        text: text.to_owned(),
        done: false,
    }
}

fn column(name: &str, cards: &[&str]) -> Column {
    Column {
        name: name.to_owned(),
        cards: cards.iter().map(|text| card(text)).collect(),
    }
}

fn board() -> Document<Board> {
    Document::new(&Board {
        title: "Plan".to_owned(),
        votes: Count(0),
        columns: [column("Todo", &["write", "review"]), column("Done", &[])]
            .into_iter()
            .collect(),
    })
}

fn edited(
    document: &Document<Board>,
    changes: impl IntoIterator<Item = Change>,
) -> Document<Board> {
    let mut document = document.clone();
    document.apply(&changes.into_iter().collect());
    document
}

fn columns(document: &Document<Board>) -> Vec<(String, Vec<String>)> {
    document
        .root()
        .columns
        .iter()
        .map(|column| {
            (
                column.name.clone(),
                column.cards.iter().map(|card| card.text.clone()).collect(),
            )
        })
        .collect()
}

fn ids(document: &Document<Board>) -> (ObjectId, ObjectId, ObjectId) {
    let root = document.root();
    let todo = &root.columns[0];
    (todo.id, root.columns[1].id, todo.cards[0].id)
}

fn undone(document: &mut Document<Board>, edit: &Edit) -> crate::Step {
    let step = document.step(edit).expect("the edit changes something");
    document.apply(edit);
    step
}

fn owned(names: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    names
        .iter()
        .map(|(name, cards)| {
            (
                (*name).to_owned(),
                cards.iter().map(|card| (*card).to_owned()).collect(),
            )
        })
        .collect()
}
