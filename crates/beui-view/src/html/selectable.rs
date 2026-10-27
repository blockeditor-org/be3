use crate::actions::Chord;
use crate::components::selectable::{COPY_KEY, SELECT_ALL_KEY};
use crate::reactive::*;
use crate::types::{Color, KeyPhase, Rect};
use beui_macros::component;

#[derive(Clone, Copy)]
pub struct SelectableState(NodeRef);

impl Default for SelectableState {
    fn default() -> Self {
        Self(NodeRef::new())
    }
}

impl SelectableState {
    pub fn selected_text(&self) -> String {
        self.0
            .try_get()
            .map(beui_core_html::nodes::selected_text)
            .unwrap_or_default()
    }

    pub fn select_all(&self) {
        if let Some(region) = self.0.try_get() {
            beui_core_html::nodes::select_all(region);
        }
    }
}

pub fn copy_selection(state: SelectableState) {
    let text = state.selected_text();
    if !text.is_empty() {
        copy_text(text);
    }
}

pub fn select_all(state: SelectableState) {
    state.select_all();
}

#[component]
pub fn Selectable(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectableState>,
    children: Child,
    #[prop(default = SelectableState::default())] state: SelectableState,
    color: Prop<Color>,
    handle_color: Prop<Color>,
    caret_width: Prop<f32>,
    #[prop(default = Some(COPY_KEY))] copy_key: Prop<Option<Chord>>,
    #[prop(default = Some(SELECT_ALL_KEY))] select_all_key: Prop<Option<Chord>>,
    on_toolbar: Callback<Option<Rect>>,
) -> NodeId {
    let _ = (color, handle_color, caret_width, on_toolbar);
    api.fill(state);
    let region = beui_core_html::document::wrap("Selectable", children);
    for property in ["-webkit-user-select", "user-select"] {
        beui_core_html::document::set_style(region, property, Some("text".to_owned()));
    }
    state.0.fill(region);
    on_key(KeyAt::Node(state.0), KeyPhase::Bubble, move |event| {
        event.press().is_some_and(|press| {
            if copy_key
                .get_untracked()
                .is_some_and(|chord| chord.matches(press))
            {
                copy_selection(state);
                return true;
            }
            if select_all_key
                .get_untracked()
                .is_some_and(|chord| chord.matches(press))
            {
                state.select_all();
                return true;
            }
            false
        })
    });
    region
}
