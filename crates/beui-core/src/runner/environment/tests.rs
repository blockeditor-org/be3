use super::*;

mod an_unknown_layer_is_refused;
mod an_unknown_platform_or_renderer_names_the_valid_ones;
mod drive_stacks_a_tap_under_each_layer_and_needs_its_socket;
mod headless_sizes_outside_the_headless_platform_are_refused;
mod layers_stack_bottom_first_over_any_platform;
mod no_renderer_needs_the_headless_platform;
mod no_variables_choose_the_native_platform_drawn_with_wgpu;
mod the_headless_platform_takes_its_window_and_screen;
mod the_native_platform_is_chosen_by_its_own_name;

fn with(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let pairs: Vec<(String, String)> = pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }
}

const NATIVE: Choice = Choice {
    platform: Base::Native,
    layers: Vec::new(),
    render: true,
};
