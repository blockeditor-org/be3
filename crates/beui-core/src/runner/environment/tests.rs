use super::*;

mod a_space_adapter_outside_space_is_refused;
mod an_unknown_adapter_or_renderer_names_the_valid_ones;
mod headless_sizes_outside_the_headless_adapter_are_refused;
mod no_renderer_needs_the_headless_adapter;
mod no_variables_choose_the_native_adapter_drawn_with_wgpu;
mod space_wraps_the_native_adapter_unless_given_another;
mod the_headless_adapter_takes_its_window_and_screen;
mod the_native_adapter_is_chosen_by_its_own_name;

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
    adapter: Base::Native,
    space: false,
    render: true,
};
