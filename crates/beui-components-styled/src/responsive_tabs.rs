use beui_macros::{component, view};

use crate::Select;
use crate::Tabs;
use crate::theme::NARROW_WIDTH;
use beui_components_unstyled::{ChoiceOption, narrower_than};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Children, List, Prop, ReadSignal, Show, clone, create_effect, create_memo,
    create_signal, set_component_state,
};

#[component]
pub fn ResponsiveTabs(
    options: Children<ChoiceOption>,
    selected: Prop<usize>,
    on_change: Callback<usize>,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
) -> NodeId {
    let options = options.into_run();
    let (selected_read, set_selected) = create_signal(selected.peek());
    create_effect(clone!(set_selected -> move || set_selected.set(selected.get())));
    set_component_state(selected_read.clone());

    let narrow = narrower_than(breakpoint);
    let wide = create_memo(clone!(narrow -> move || !narrow.get()));
    let highlighted = create_memo(clone!(selected_read -> move || Some(selected_read.get())));

    let tab_options = options.clone();
    let tab_selected = selected_read.clone();
    let tab_change = on_change.clone();
    let tab_set = set_selected.clone();

    view! {
        <List spacing=0.0>
            <Show
                condition={wide}
                then={move || clone!(tab_options tab_selected tab_set tab_change -> view! {
                    <Tabs
                        options={tab_options}
                        selected={tab_selected}
                        on_change={move |index| {
                            tab_set.set(index);
                            tab_change.call(index);
                        }}
                    />
                })}
            />
            <Show
                condition={narrow}
                then={move || clone!(options highlighted set_selected on_change -> view! {
                    <Select
                        options
                        selected={highlighted}
                        on_change={move |index: Option<usize>| {
                            if let Some(index) = index {
                                set_selected.set(index);
                                on_change.call(index);
                            }
                        }}
                    />
                })}
            />
        </List>
    }
}

pub fn responsive_tabs_selected(document: &Document, tabs: NodeId) -> usize {
    document.component_state::<ReadSignal<usize>>(tabs).get()
}
