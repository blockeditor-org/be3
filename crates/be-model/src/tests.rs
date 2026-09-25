use crate::{
    Anchor, Bounds, Change, Count, Document, Edit, Grid, List, Map, Model, ObjectId, Paint, Touched,
};

mod a_burst_of_sets_to_one_field_undoes_as_one_step;
mod a_card_cannot_be_moved_after_itself_or_into_something_that_is_not_a_list;
mod a_card_edited_after_a_move_is_edited_where_it_went;
mod a_card_inserted_after_one_someone_removed_at_once_lands_where_that_one_was;
mod a_card_moved_to_different_columns_on_each_side_conflicts_and_goes_where_ours_put_it;
mod a_card_moved_to_the_same_place_on_both_sides_merges_cleanly;
mod a_card_removed_on_both_sides_merges_without_a_conflict;
mod a_column_removed_on_one_side_stays_removed_when_the_other_edits_a_card_in_it;
mod a_column_removed_on_one_side_stays_removed_when_the_other_fills_it;
mod a_column_removed_on_one_side_stays_removed_when_the_other_moves_a_card_into_it;
mod a_document_reads_back_what_it_was_built_from;
mod a_document_round_trips_every_kind_of_field_through_bytes;
mod a_field_added_to_a_type_after_a_document_was_saved_can_be_set;
mod a_field_changed_on_one_side_merges_to_that_change_from_either_side;
mod a_grid_grown_the_same_way_on_both_sides_keeps_both_paints;
mod a_grid_reshaped_differently_on_each_side_conflicts_and_keeps_ours;
mod a_key_removed_on_one_side_and_changed_on_the_other_stays_removed;
mod a_move_that_changes_nothing_has_no_step;
mod a_node_cannot_move_inside_itself;
mod a_paint_the_other_side_cropped_away_counts_as_a_conflict;
mod a_reshape_keeps_every_cell_at_its_coordinates;
mod an_edit_that_changes_nothing_has_no_step;
mod an_edit_touches_its_field_and_what_holds_it;
mod an_insert_into_a_removed_column_changes_nothing;
mod an_insert_reusing_an_id_already_in_the_document_is_refused;
mod both_sides_setting_a_field_to_the_same_value_merge_without_a_conflict;
mod cards_inserted_at_the_same_place_on_both_sides_merge_to_both_in_that_place;
mod cards_inserted_at_the_start_and_the_end_at_once_keep_their_ends;
mod count_changes_made_at_once_add_up_in_either_order;
mod counts_merge_by_adding_both_sides;
mod counts_saturate_instead_of_wrapping;
mod grids_merge_cell_by_cell_and_keep_coordinates_across_a_resize;
mod inserting_touches_the_list_and_everything_inserted;
mod map_entries_merge_key_by_key;
mod merging_a_document_with_itself_changes_nothing;
mod merging_a_move_on_one_side_with_an_edit_on_the_other_keeps_both;
mod nodes_moved_into_each_other_on_each_side_merge_without_a_cycle;
mod one_field_set_on_both_sides_conflicts_and_keeps_ours;
mod one_step_undoes_every_change_an_edit_made;
mod painting_outside_a_grid_or_with_the_wrong_cell_size_changes_nothing;
mod puts_to_different_keys_at_once_both_land;
mod redo_leaves_a_field_someone_else_changed_after_the_undo;
mod removing_a_column_while_the_other_side_moves_a_card_out_of_it_keeps_only_that_card;
mod removing_touches_the_list_and_everything_that_was_inside;
mod reorders_of_different_cards_on_each_side_merge_to_both;
mod setting_a_field_of_a_removed_card_changes_nothing;
mod strokes_into_different_grids_undo_separately;
mod the_same_value_put_under_a_key_on_both_sides_merges_without_a_conflict;
mod two_inserts_after_the_same_card_at_once_both_land_after_it;
mod undo_leaves_a_field_someone_else_changed_since;
mod undo_of_map_entries_restores_only_what_nobody_changed_since;
mod undo_puts_a_removed_column_back_with_its_cards;
mod undoing_a_move_leaves_a_card_someone_else_moved_since;
mod undoing_a_move_puts_the_card_back_where_it_was;
mod undoing_a_put_of_a_new_key_removes_it_unless_someone_changed_it_since;
mod undoing_a_removal_puts_the_card_back_after_its_neighbour_was_removed_since;
mod undoing_an_add_takes_back_only_that_add;
mod undoing_an_edit_to_a_card_removed_since_changes_nothing;
mod undoing_an_insert_removes_the_card_and_redo_puts_it_back;
mod undoing_paint_and_a_crop_restores_only_what_nobody_changed_since;
mod undoing_the_removal_of_several_cards_restores_their_order;

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

fn card_id(document: &Document<Board>, text: &str) -> ObjectId {
    document
        .root()
        .columns
        .iter()
        .flat_map(|column| {
            column
                .cards
                .iter()
                .map(|card| (card.id, card.text.clone()))
                .collect::<Vec<_>>()
        })
        .find(|(_, held)| held == text)
        .map(|(id, _)| id)
        .expect("the card is on the board")
}

fn sheet(cells: &[(u32, &str)]) -> Document<Sheet> {
    Document::new(&Sheet {
        cells: cells
            .iter()
            .map(|(key, value)| (*key, (*value).to_owned()))
            .collect(),
    })
}

fn put(document: &Document<Sheet>, cells: &[(u32, Option<&str>)]) -> Document<Sheet> {
    let mut document = document.clone();
    for (key, value) in cells {
        document.apply(
            &Sheet::CELLS
                .put(ObjectId::ROOT, key, value.map(str::to_owned).as_ref())
                .into(),
        );
    }
    document
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Picture {
    pixels: Grid<[u8; 1]>,
}

fn picture(bounds: Bounds) -> Document<Picture> {
    Document::new(&Picture {
        pixels: Grid::new(bounds),
    })
}

fn painted(document: &Document<Picture>, cells: &[(i32, i32, u8)]) -> Document<Picture> {
    let mut changed = document.clone();
    changed.apply(
        &Picture::PIXELS
            .paint(
                ObjectId::ROOT,
                cells.iter().map(|(x, y, value)| (*x, *y, [*value])),
            )
            .into(),
    );
    changed
}

fn pixel(document: &Document<Picture>, x: i32, y: i32) -> Option<u8> {
    document.root().pixels.get(x, y).map(|[value]| value)
}
