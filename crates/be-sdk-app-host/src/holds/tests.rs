use super::*;

mod a_closed_anchored_view_and_a_hidden_view_stay_until_their_frame_is_drawn;
mod a_frame_carries_damage_only_when_every_present_since_the_last_one_reported_it;
mod a_layout_s_children_wait_for_the_present_that_follows_it;

fn placement(screen: u64, surface: u32) -> ScreenPlacement {
    ScreenPlacement {
        screen: ScreenId(screen),
        instance: EditorInstanceId(1),
        view: ViewId(screen),
        surface,
        width: 10,
        height: 10,
        scale_factor_millis: 1000,
        raster_millis: 1000,
    }
}

fn layout(screens: &[u64]) -> ScreenLayout {
    ScreenLayout {
        screens: screens
            .iter()
            .map(|screen| placement(*screen, *screen as u32))
            .collect(),
    }
}
