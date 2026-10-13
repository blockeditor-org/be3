use super::{
    LOADED, Pos, SeqOp, Sequence, Span, Splice,
    fuzz::{check, order},
};

mod a_delete_leaves_text_inserted_inside_it_at_the_same_time;
mod a_merged_delete_still_lands_beside_positions_a_refused_insert_left_unknown;
mod a_move_into_itself_or_with_its_ends_out_of_order_changes_nothing;
mod a_move_takes_text_inserted_inside_it_along;
mod a_refused_replace_still_uses_up_its_offsets;
mod an_insert_after_a_deleted_element_lands_where_it_was;
mod an_insert_with_the_wrong_start_or_an_unknown_anchor_is_refused;
mod bytes_shown_again_join_the_bytes_held_after_them;
mod bytes_that_are_not_utf8_are_kept_as_they_are;
mod deleting_one_byte_at_a_time_leaves_one_deleted_run;
mod inserts_after_the_same_element_put_the_later_one_first;
mod loaded_items_read_back_as_one_fragment;
mod operations_with_numbers_out_of_range_change_nothing;
mod random_bytes_drive_every_replica_to_the_reference;
mod runs_list_every_fragment_with_what_it_holds;
mod session_state_round_trips_positions_and_tombstones;
mod splices_report_visible_coordinates;
mod the_saved_form_is_only_the_visible_items;
mod two_people_checking_the_same_box_check_it_once;
mod typing_and_deleting_absorb_into_one_operation;
mod typing_extends_one_fragment;
mod undoing_a_delete_brings_back_the_same_positions;
mod undoing_a_move_puts_the_range_back;
mod undoing_a_replace_swaps_back_and_redo_swaps_again;

const ALICE: u64 = 1;
const BOB: u64 = 2;

fn loaded(text: &str) -> Sequence<u8> {
    Sequence::from_items(text.as_bytes().to_vec())
}

fn text(sequence: &Sequence<u8>) -> String {
    String::from_utf8_lossy(&sequence.items()).into_owned()
}

fn applied(sequence: &mut Sequence<u8>, op: &SeqOp<u8>) -> Vec<Splice> {
    sequence.apply(op).expect("the operation applies")
}

fn typed(sequence: &mut Sequence<u8>, client: u64, index: usize, typed: &str) -> SeqOp<u8> {
    let op = sequence
        .insert(client, index, typed.as_bytes().to_vec())
        .expect("there is something to insert");
    applied(sequence, &op);
    op
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        match bound {
            0 => 0,
            _ => (self.next() % bound as u64) as usize,
        }
    }
}
