use beui_macros::{component, view};

use crate::Select;
use crate::Tabs;
use crate::theme::NARROW_WIDTH;
use beui_components_unstyled::{ChoiceOption, narrower_than};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[component]
pub fn ResponsiveTabs(
    options: Children<ChoiceOption>,
    selected: Prop<usize>,
    on_change: Callback<usize>,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
) -> NodeId {
    let options = options.into_run();
    let current = create_signal(selected.get_untracked());
    create_effect(move || current.set(selected.get()));
    set_component_state(current.read_only());

    let narrow = narrower_than(breakpoint);
    let wide = create_memo(move || !narrow.get());
    let highlighted = create_memo(move || Some(current.get()));

    let tab_options = options.clone();
    let tab_selected = current;
    let tab_change = on_change;
    let tab_set = current;

    view! {
        <List spacing=0.0>
            <Show
                condition={wide}
                then={move || { let tab_options = tab_options.clone(); view! {
                    <Tabs
                        options={tab_options}
                        selected={tab_selected}
                        on_change={move |index| {
                            tab_set.set(index);
                            tab_change.call(index);
                        }}
                    />
                } }}
            />
            <Show
                condition={narrow}
                then={move || { let options = options.clone(); view! {
                    <Select
                        options
                        selected={highlighted}
                        on_change={move |index: Option<usize>| {
                            if let Some(index) = index {
                                current.set(index);
                                on_change.call(index);
                            }
                        }}
                    />
                } }}
            />
        </List>
    }
}

pub fn responsive_tabs_selected(document: &Document, tabs: NodeId) -> usize {
    document.component_state::<ReadSignal<usize>>(tabs).get()
}
