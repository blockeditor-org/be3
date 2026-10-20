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
    #[prop(default = 0)] selected: Binding<usize>,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
) -> NodeId {
    let options = options.into_run();
    set_component_state(selected);

    let narrow = narrower_than(breakpoint);
    let wide = create_memo(move || !narrow.get());
    let highlighted = Binding::new(
        Prop::dynamic(move || Some(selected.get())),
        move |index: Option<usize>| {
            if let Some(index) = index {
                selected.set(index);
            }
        },
    );

    let tab_options = options.clone();

    view! {
        <List spacing=0.0>
            <Show
                condition={wide}
                then={move || { let tab_options = tab_options.clone(); view! {
                    <Tabs
                        options={tab_options}
                        selected
                    />
                } }}
            />
            <Show
                condition={narrow}
                then={move || { let options = options.clone(); view! {
                    <Select
                        options
                        selected={highlighted}
                    />
                } }}
            />
        </List>
    }
}

pub fn responsive_tabs_selected(document: &Document, tabs: NodeId) -> usize {
    document.component_state::<Binding<usize>>(tabs).get()
}
