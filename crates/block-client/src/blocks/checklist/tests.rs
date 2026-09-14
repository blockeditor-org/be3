use block::Block;
use uuid::Uuid;

use super::{Checklist, ChecklistItem, ChecklistOperation};

mod adding_the_same_item_twice_keeps_one_copy;
mod clear_done_keeps_open_items;
mod operations_for_unknown_items_are_ignored;
mod serialization_round_trip;
