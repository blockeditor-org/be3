use uuid::Uuid;

use super::{CreateView, PickerView, creation_surface, publish, views};
use crate::surfaces::SurfaceId;

mod a_picker_opened_from_a_creation_dialog_is_shown_above_it;
mod a_picker_with_nothing_to_show_is_not_shown;

fn creating(id: Uuid, depth: usize) -> PickerView {
    PickerView {
        id,
        depth,
        choose: None,
        create: Some(CreateView {
            title: "Game".to_owned(),
            working: false,
            ready: false,
            dialog: true,
        }),
        error: None,
    }
}
