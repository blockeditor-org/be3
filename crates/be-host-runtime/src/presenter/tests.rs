use be_sdk_api::{EditorInstanceId, ViewId};

use super::*;
mod a_child_over_an_anchored_view_is_lifted_and_a_pane_beside_it_is_not;
mod a_frame_published_after_its_damage_was_read_is_redrawn_next;
mod a_layout_that_lands_before_the_present_of_its_size_shows_the_last_frame_at_its_own_size;
mod a_preview_is_fitted_to_each_place_it_is_shown;
mod a_surface_painted_before_the_layout_that_brought_it_is_published_still_shows;
mod a_view_laid_out_at_a_fraction_of_a_pixel_shows_its_painting_as_cut;
mod an_apps_painting_is_shown_at_its_own_size_whatever_size_its_view_is_laid_out_at;
mod an_apps_textures_are_let_go_once_its_layout_no_longer_holds_them;
mod surface_damage_reaches_the_pieces_of_the_screen_it_falls_on;

const WHOLE: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
fn placement(width: u32, height: u32) -> ScreenPlacement {
    ScreenPlacement {
        screen: ScreenId(1),
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        surface: 4,
        width,
        height,
        scale_factor_millis: 2000,
        raster_millis: 2000,
    }
}
